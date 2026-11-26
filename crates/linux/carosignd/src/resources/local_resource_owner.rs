use super::{
    BoxFuture, CleanupScope, ExitCleanup, LeaseId, MutationLeaseVerifier, QuiesceAssessment,
    ResourceDenial, ResourceLease, ResourceOwner, ResourcePort, ResourceSet,
};

/// Recording the outputs of one demon process. Each resource holds one operation.
/// The unknown outcome of the cleaning leaves the issue occupied until the processes stop.
#[derive(Clone)]
pub struct LocalResourceOwner {
    startup_fenced_operations:
        Option<std::sync::Arc<std::collections::BTreeSet<carosign_core::ids::OperationId>>>,
    pub(super) scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
}

impl Default for LocalResourceOwner {
    fn default() -> Self {
        Self {
            startup_fenced_operations: Some(Default::default()),
            scheduler: std::sync::Arc::new(std::sync::Mutex::new(
                carosign_runtime::scheduler::Scheduler::new(),
            )),
        }
    }
}

impl LocalResourceOwner {
    /// Experimental recovery: The source is held by the scheduler.
    /// By itself, it does not connect storage to the service and does not allow Bind.
    pub fn from_initial_fence_store(
        store: carosign_persistence::redb::backend::InitialFenceStore,
    ) -> Result<Self, ResourceDenial> {
        let fences = store
            .fences_with_operations()
            .map_err(|_| ResourceDenial::Unavailable)?;
        if !fences.resources.is_empty() {
            eprintln!(
                "carosignd: восстановлены ограждения ресурсов: {:02x?}, операции: {:02x?}",
                fences
                    .resources
                    .iter()
                    .map(|id| id.as_bytes())
                    .collect::<Vec<_>>(),
                fences.operations.as_ref().map(|operations| operations
                    .iter()
                    .map(|id| id.as_bytes())
                    .collect::<Vec<_>>()),
            );
        }
        let mut scheduler = carosign_runtime::scheduler::Scheduler::new();
        scheduler
            .restore_durable_fences(fences.resources, std::sync::Arc::new(store))
            .map_err(|_| ResourceDenial::Unavailable)?;
        Ok(Self {
            startup_fenced_operations: fences.operations.map(std::sync::Arc::new),
            scheduler: std::sync::Arc::new(std::sync::Mutex::new(scheduler)),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_shared_fence_store(
        store: std::sync::Arc<carosign_persistence::redb::backend::InitialFenceStore>,
    ) -> Result<Self, ResourceDenial> {
        let fences = store
            .fences_with_operations()
            .map_err(|_| ResourceDenial::Unavailable)?;
        let mut scheduler = carosign_runtime::scheduler::Scheduler::new();
        scheduler
            .restore_durable_fences(fences.resources, store)
            .map_err(|_| ResourceDenial::Unavailable)?;
        Ok(Self {
            startup_fenced_operations: fences.operations.map(std::sync::Arc::new),
            scheduler: std::sync::Arc::new(std::sync::Mutex::new(scheduler)),
        })
    }

    pub(crate) fn has_recovered_fence(&self, set: &ResourceSet) -> std::io::Result<bool> {
        self.scheduler
            .lock()
            .map(|scheduler| scheduler.has_recovered_fence(set))
            .map_err(|_| std::io::Error::other("планировщик недоступен"))
    }

    pub(crate) fn startup_cleanup_confirmed(
        &self,
        operation: carosign_core::ids::OperationId,
    ) -> bool {
        self.startup_fenced_operations
            .as_ref()
            .is_some_and(|fenced| !fenced.contains(&operation))
    }

    /// Gives out the resources of the modifying operation with the corresponding mark in the
    /// general scheduler; the issue does not arm the command and does not give authority.
    pub async fn acquire_mutation(
        &self,
        owner: ResourceOwner,
        set: &ResourceSet,
    ) -> Result<ResourceLease, ResourceDenial> {
        self.acquire_owned(owner, set, true)
    }

    fn acquire_owned(
        &self,
        owner: ResourceOwner,
        set: &ResourceSet,
        mutating: bool,
    ) -> Result<ResourceLease, ResourceDenial> {
        use carosign_core::ids::IdSource;
        use carosign_runtime::scheduler::{GrantResult, Priority, Request};
        if set
            .claims()
            .iter()
            .any(|claim| claim.mode != carosign_core::resource::AccessMode::Exclusive)
        {
            return Err(ResourceDenial::Denied);
        }
        let boot = carosign_core::ids::BootId::from_bytes(
            carosign_linux::sys::boot_id().map_err(|_| ResourceDenial::Unavailable)?,
        )
        .map_err(|_| ResourceDenial::Unavailable)?;
        let now = carosign_linux::sys::boottime_ns().map_err(|_| ResourceDenial::Unavailable)?;
        let id = carosign_linux::ids::KernelIds
            .new_lease_id()
            .map_err(|_| ResourceDenial::Unavailable)?;
        let mut scheduler = self
            .scheduler
            .lock()
            .map_err(|_| ResourceDenial::Unavailable)?;
        scheduler.set_authenticator_epoch(owner.authenticator);
        if let Some(holder) = set
            .claims()
            .iter()
            .find_map(|claim| scheduler.holder_of(claim.id))
        {
            use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

            let operation_tag = BuildHasherDefault::<DefaultHasher>::new();
            eprintln!(
                "carosignd: аренда занята: операция={:016x}, владелец={:016x}, изменение={mutating}",
                operation_tag.hash_one(owner.operation),
                operation_tag.hash_one(holder),
            );
            return Err(ResourceDenial::Busy);
        }
        let request = Request {
            owner,
            priority: Priority::SystemAuth,
            set: set.clone(),
            since: carosign_core::operation::context::BootInstant::new(boot, now),
            mutating,
        };
        match scheduler.acquire(request, id) {
            GrantResult::Granted { .. } => Ok(ResourceLease::issued(id, owner, set.clone())),
            GrantResult::Queued => {
                scheduler.cancel_queued(owner.operation, id);
                eprintln!("carosignd: планировщик поставил аренду в очередь, изменение={mutating}");
                Err(ResourceDenial::Busy)
            }
            GrantResult::Denied => {
                eprintln!("carosignd: планировщик отклонил аренду, изменение={mutating}");
                Err(ResourceDenial::Unavailable)
            }
        }
    }
    /// The same scheduler used by the owner of the operations.
    pub fn scheduler(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>> {
        self.scheduler.clone()
    }
    /// Verifies the issuance with a live record before sending a command to the broker.
    pub fn validate_live(&self, lease: &ResourceLease) -> bool {
        self.scheduler.lock().is_ok_and(|scheduler| {
            scheduler.holds_live_scope(lease.lease_id(), lease.owner(), lease.resource_set())
        })
    }

    /// Serializes issuance checks, stable fencing and Bind revocation/era transfer. The
    /// scheduler lock is held for a time-limited shipment; the fence remains on an uncertain
    /// outcome.
    pub(crate) fn with_serialized_binding<T>(
        &self,
        scope: &CleanupScope,
        durable: bool,
        send: impl FnOnce() -> T,
    ) -> Result<T, ResourceDenial> {
        let mut scheduler = self
            .scheduler
            .lock()
            .map_err(|_| ResourceDenial::Unavailable)?;
        if !scheduler.holds_live_scope(scope.lease_id(), scope.owner(), scope.resource_set()) {
            return Err(ResourceDenial::Denied);
        }
        if durable {
            scheduler
                .prepare_durable_fence(scope.lease_id(), scope.owner(), scope.resource_set())
                .map_err(|_| ResourceDenial::Unavailable)?;
        }
        Ok(send())
    }

    /// Checks the current worker's two child processes through `waitpid`.
    /// An open channel or a live mediator does not mean a complete cleaning.
    #[cfg(test)]
    pub(crate) fn confirm_stopped(
        &self,
        worker_id: carosign_core::ids::WorkerInstanceId,
        worker: &mut std::process::Child,
        broker: &mut std::process::Child,
        cleanup: ExitCleanup,
    ) -> Result<(), ResourceDenial> {
        let worker_exited = worker
            .try_wait()
            .map_err(|_| ResourceDenial::Unavailable)?
            .is_some();
        let broker_exited = broker
            .try_wait()
            .map_err(|_| ResourceDenial::Unavailable)?
            .is_some();
        self.confirm_exited(worker_id, worker_exited, broker_exited, cleanup)
    }

    pub(crate) fn confirm_exited(
        &self,
        worker_id: carosign_core::ids::WorkerInstanceId,
        worker_exited: bool,
        broker_exited: bool,
        cleanup: ExitCleanup,
    ) -> Result<(), ResourceDenial> {
        if !worker_exited || !broker_exited {
            return Err(ResourceDenial::Busy);
        }
        if cleanup == ExitCleanup::ProcessOnly {
            self.scheduler
                .lock()
                .map_err(|_| ResourceDenial::Unavailable)?
                .mark_processes_exited(worker_id);
        }
        Ok(())
    }
}

impl ResourcePort for LocalResourceOwner {
    fn acquire_for<'a>(
        &'a self,
        owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move { self.acquire_owned(owner, set, false) })
    }

    fn acquire<'a>(
        &'a self,
        _set: &'a ResourceSet,
        _lease_id: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        // A request without an owner cannot be reliably matched to a process.
        Box::pin(async { Err(ResourceDenial::Denied) })
    }

    fn release(&self, lease: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        Box::pin(async move {
            let Ok(mut scheduler) = self.scheduler.lock() else {
                return QuiesceAssessment::Uncertain;
            };
            if !scheduler.release_after_processes_exit(
                lease.lease_id(),
                lease.owner(),
                lease.resource_set(),
            ) {
                return QuiesceAssessment::Uncertain;
            }
            QuiesceAssessment::Quiesced
        })
    }
}

impl MutationLeaseVerifier for LocalResourceOwner {
    fn validate<'a>(
        &'a self,
        scope: &'a CleanupScope,
    ) -> BoxFuture<'a, Result<(), ResourceDenial>> {
        Box::pin(async move {
            let scheduler = self
                .scheduler
                .lock()
                .map_err(|_| ResourceDenial::Unavailable)?;
            if scheduler.holds_live_scope(scope.lease_id(), scope.owner(), scope.resource_set()) {
                Ok(())
            } else {
                Err(ResourceDenial::Denied)
            }
        })
    }
}
