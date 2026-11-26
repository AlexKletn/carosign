use super::{
    BoxFuture, CleanupPortFailure, CleanupScope, ExitCleanup, LocalResourceOwner, QuiescedLease,
    TrustedCleanupBroker, relay_scope,
};

/// Removes the issuance by stopping tied processes before release.
pub(crate) struct SupervisorCleanup {
    supervisor: std::sync::Mutex<crate::provider_supervisor::ProviderSupervisor>,
    resources: std::sync::Arc<LocalResourceOwner>,
    pub(super) stopped_scope: std::sync::Mutex<Option<CleanupScope>>,
    control: std::sync::Mutex<Option<crate::broker_control::BrokerControl>>,
    issued_scope: Option<CleanupScope>,
    interrupted: std::sync::atomic::AtomicBool,
    preparation_revoker: std::sync::Mutex<Option<std::sync::Arc<dyn Fn() -> bool + Send + Sync>>>,
    enrollment_admission: std::sync::Mutex<
        Option<
            std::sync::Weak<carosign_runtime::enrollment_admission::SerializedEnrollmentAdmission>,
        >,
    >,
}

impl SupervisorCleanup {
    /// Interrupts the processes after cancellation, without releasing the fenced issue.
    pub(crate) fn interrupt(&self) -> std::io::Result<()> {
        use std::sync::atomic::Ordering;
        self.interrupted.store(true, Ordering::SeqCst);
        let admission = self
            .enrollment_admission
            .lock()
            .map(|slot| slot.as_ref().and_then(std::sync::Weak::upgrade));
        let authorization_revoked = match admission {
            Ok(Some(admission)) => {
                // A busy call will be withdrawn upon return; this does not prove peace.
                admission.revoke().is_ok()
            }
            Ok(None) => self
                .preparation_revoker
                .lock()
                .map(|slot| slot.clone())
                .is_ok_and(|revoke| {
                    revoke.is_none_or(|revoke| {
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| revoke()))
                            .unwrap_or(false)
                    })
                }),
            Err(_) => false,
        };
        let mut stopped = self
            .stopped_scope
            .lock()
            .map_err(|_| std::io::Error::other("состояние уборки недоступно"))?;
        let mut supervisor = self
            .supervisor
            .lock()
            .map_err(|_| std::io::Error::other("supervisor недоступен"))?;
        let proof = if let Some(scope) = self.issued_scope.as_ref()
            && supervisor.exit_cleanup() == ExitCleanup::HardwareUnconfirmed
            && supervisor.has_exclusive_owner()
            && supervisor.worker_id() == scope.owner().worker
            && self.resources.scheduler.lock().is_ok_and(|scheduler| {
                scheduler.holds_scope(scope.lease_id(), scope.owner(), scope.resource_set())
            }) {
            self.revoke_descriptors(relay_scope(scope), std::time::Duration::from_secs(5))
                .is_ok()
        } else {
            false
        };
        supervisor.stop_and_confirm(&self.resources)?;
        if proof && authorization_revoked {
            *stopped = self.issued_scope.clone();
        } else if !authorization_revoked {
            *stopped = None;
        }
        if authorization_revoked {
            Ok(())
        } else {
            Err(std::io::Error::other("отзыв регистрации не подтверждён"))
        }
    }
    pub(crate) fn new(
        mut supervisor: crate::provider_supervisor::ProviderSupervisor,
        resources: std::sync::Arc<LocalResourceOwner>,
    ) -> Self {
        let control = supervisor.take_control();
        Self {
            supervisor: std::sync::Mutex::new(supervisor),
            resources,
            stopped_scope: std::sync::Mutex::new(None),
            control: std::sync::Mutex::new(control),
            issued_scope: None,
            interrupted: std::sync::atomic::AtomicBool::new(false),
            preparation_revoker: std::sync::Mutex::new(None),
            enrollment_admission: std::sync::Mutex::new(None),
        }
    }

    pub(crate) fn install_preparation_revoker(
        &self,
        revoke: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> std::io::Result<()> {
        use std::sync::atomic::Ordering;
        let mut slot = self
            .preparation_revoker
            .lock()
            .map_err(|_| std::io::Error::other("отзыв недоступен"))?;
        if self.interrupted.load(Ordering::SeqCst) || slot.is_some() {
            return Err(std::io::Error::other("операция прервана"));
        }
        *slot = Some(revoke.clone());
        if self.interrupted.load(Ordering::SeqCst) {
            drop(slot);
            let _ = revoke();
            return Err(std::io::Error::other("операция прервана"));
        }
        Ok(())
    }

    pub(crate) fn install_enrollment_admission(
        &self,
        admission: &std::sync::Arc<
            carosign_runtime::enrollment_admission::SerializedEnrollmentAdmission,
        >,
    ) -> std::io::Result<()> {
        use std::sync::atomic::Ordering;
        let mut slot = self
            .enrollment_admission
            .lock()
            .map_err(|_| std::io::Error::other("допуск недоступен"))?;
        if self.interrupted.load(Ordering::SeqCst) || slot.is_some() {
            return Err(std::io::Error::other("операция прервана"));
        }
        *slot = Some(std::sync::Arc::downgrade(admission));
        if self.interrupted.load(Ordering::SeqCst) {
            let _ = admission.revoke();
            return Err(std::io::Error::other("операция прервана"));
        }
        Ok(())
    }

    pub(crate) fn new_for_scope(
        supervisor: crate::provider_supervisor::ProviderSupervisor,
        resources: std::sync::Arc<LocalResourceOwner>,
        scope: CleanupScope,
    ) -> Self {
        let mut cleanup = Self::new(supervisor, resources);
        cleanup.issued_scope = Some(scope);
        cleanup
    }

    /// The channel is available regardless of the occupancy of the supervisor, and the answer
    /// applies only to the descriptors withdrawn, not to the outcome of the operation.
    pub(crate) fn revoke_descriptors(
        &self,
        scope: carosign_protocol::carosign::broker_relay::v1::Scope,
        timeout: std::time::Duration,
    ) -> std::io::Result<()> {
        let control = self
            .control
            .lock()
            .map_err(|_| std::io::Error::other("канал отзыва недоступен"))?
            .take()
            .ok_or_else(|| std::io::Error::other("канал отзыва недоступен"))?;
        control.revoke(scope, timeout)
    }

    pub(crate) fn confirms_worker(&self, worker: carosign_core::ids::WorkerInstanceId) -> bool {
        self.supervisor
            .lock()
            .is_ok_and(|mut supervisor| supervisor.confirms_worker(worker))
    }
}

impl TrustedCleanupBroker for SupervisorCleanup {
    fn processes_stopped(&self, scope: &CleanupScope) -> bool {
        self.supervisor
            .lock()
            .is_ok_and(|mut supervisor| supervisor.processes_stopped(scope.owner().worker))
            && self.resources.scheduler.lock().is_ok_and(|scheduler| {
                scheduler.holds_scope(scope.lease_id(), scope.owner(), scope.resource_set())
            })
    }
    fn quiesce<'a>(
        &'a self,
        scope: &'a CleanupScope,
    ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        Box::pin(async move {
            let held = self
                .resources
                .scheduler
                .lock()
                .map_err(|_| CleanupPortFailure::Unavailable)?
                .holds_scope(scope.lease_id(), scope.owner(), scope.resource_set());
            if !held {
                return Err(CleanupPortFailure::Uncertain);
            }
            let mut stopped = self
                .stopped_scope
                .lock()
                .map_err(|_| CleanupPortFailure::Unavailable)?;
            if let Some(confirmed) = stopped.as_ref() {
                return if confirmed == scope {
                    Ok(scope.clone())
                } else {
                    Err(CleanupPortFailure::Uncertain)
                };
            }
            let mut supervisor = self
                .supervisor
                .lock()
                .map_err(|_| CleanupPortFailure::Unavailable)?;
            if supervisor.worker_id() != scope.owner().worker {
                return Err(CleanupPortFailure::Uncertain);
            }
            let exclusive_quiesced = if supervisor.exit_cleanup()
                == ExitCleanup::HardwareUnconfirmed
                && supervisor.has_exclusive_owner()
            {
                // The answer confirms the end of the hardware operation and the withdrawal of
                // the exclusive issue, but not the outcome of the mutation in the journal.
                self.revoke_descriptors(relay_scope(scope), std::time::Duration::from_secs(5))
                    .is_ok()
            } else {
                false
            };
            supervisor
                .stop_and_confirm(&self.resources)
                .map_err(|_| CleanupPortFailure::Uncertain)?;
            if supervisor.exit_cleanup() == ExitCleanup::HardwareUnconfirmed && !exclusive_quiesced
            {
                return Err(CleanupPortFailure::Uncertain);
            }
            *stopped = Some(scope.clone());
            Ok(scope.clone())
        })
    }

    fn release<'a>(
        &'a self,
        scope: &'a CleanupScope,
        proof: QuiescedLease,
    ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        Box::pin(async move {
            let mut stopped = self
                .stopped_scope
                .lock()
                .map_err(|_| CleanupPortFailure::Unavailable)?;
            if stopped.as_ref() != Some(scope) || proof.scope() != scope {
                return Err(CleanupPortFailure::Uncertain);
            }
            *stopped = None;
            Ok(scope.clone())
        })
    }
}
