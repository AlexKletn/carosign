use super::{
    Arc, AtomicBool, ControlError, Duration, MetadataStore, MutationId, OperationId, Ordering,
    PreparedMutation, TimeSource,
};

pub(crate) struct MutationControl {
    operation: OperationId,
    rollback_requested: Arc<AtomicBool>,
    control: Arc<crate::control_watch::ControlOwner>,
    cleanup: Arc<crate::resources::SupervisorCleanup>,
    // Reader is completed before the observer, so as not to lose the confirmation.
    _reader: crate::cancel_reader::CancelReader,
    _watch: crate::control_watch::WatchHandle,
    _sender: crate::control_watch::RevocationSender,
}

impl MutationControl {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        connection: &crate::server::Connection,
        operation: OperationId,
        registry: Arc<crate::operations::OperationRegistry>,
        control: Arc<crate::control_watch::ControlOwner>,
        policy: &crate::linux_policy::LinuxPolicy,
        prepared: &PreparedMutation,
        cleanup: Arc<crate::resources::SupervisorCleanup>,
        clock: Arc<dyn TimeSource>,
        lifetime: Duration,
        rollback_enabled: bool,
    ) -> Result<Self, ControlError> {
        let policy_watch = policy
            .operation_watch_for(&prepared.caller, &prepared.grant)
            .map_err(|_| ControlError::Watch)?;
        let (watch, sender) = control
            .start_observing(
                registry.clone(),
                operation,
                clock,
                lifetime,
                Some(policy_watch),
            )
            .map_err(|_| ControlError::Watch)?;
        let rollback_requested = Arc::new(AtomicBool::new(false));
        let reader = crate::cancel_reader::CancelReader::start(
            connection,
            operation,
            &registry,
            sender.clone(),
            control.clone(),
            cleanup.clone(),
            rollback_enabled.then(|| rollback_requested.clone()),
        )
        .map_err(|_| ControlError::Reader)?;
        Ok(Self {
            operation,
            rollback_requested,
            control,
            cleanup,
            _reader: reader,
            _watch: watch,
            _sender: sender,
        })
    }

    /// The revocation or cancellation has already been registered by the management owner.
    pub(crate) fn interrupted(&self) -> bool {
        self.control.ready(self.operation) || self._reader.pending_cancel_or_closed()
    }

    pub(crate) fn rollback_requested(&self) -> bool {
        self.rollback_requested.load(Ordering::Acquire)
    }

    /// Interrupts processes and fixes an uncertain outcome after arm.
    /// The error leaves the area and the queue closed; in no event does it follow that the
    /// cancellation command reached the provider.
    pub(crate) fn reconcile<S: MetadataStore>(
        &self,
        store: &S,
        mutation: MutationId,
    ) -> Result<(), ControlError> {
        let stopped = self.cleanup.interrupt().is_ok();
        let durable = (|| {
            let mut tx = store.begin_write()?;
            carosign_persistence::mutation_store::stage_needs_reconciliation_v4(
                &mut *tx,
                mutation,
                self.operation,
            )?;
            tx.commit_durable().map(|_| ())
        })()
        .is_ok();
        if stopped && durable {
            Ok(())
        } else {
            Err(ControlError::Uncertain)
        }
    }
}
