use super::*;

pub(super) struct FailingIds {
    pub(super) inner: Ids,
    pub(super) fail_operation: bool,
    pub(super) calls: std::sync::atomic::AtomicUsize,
}

impl IdSource for FailingIds {
    fn new_operation_id(&self) -> Result<OperationId, carosign_core::ids::IdUnavailable> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail_operation {
            Err(carosign_core::ids::IdUnavailable)
        } else {
            self.inner.new_operation_id()
        }
    }
    fn new_lease_id(&self) -> Result<LeaseId, carosign_core::ids::IdUnavailable> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail_operation {
            self.inner.new_lease_id()
        } else {
            Err(carosign_core::ids::IdUnavailable)
        }
    }
    fn new_mutation_id(&self) -> Result<MutationId, carosign_core::ids::IdUnavailable> {
        self.inner.new_mutation_id()
    }
    fn new_command_id(&self) -> Result<CommandId, carosign_core::ids::IdUnavailable> {
        self.inner.new_command_id()
    }
    fn new_worker_instance_id(
        &self,
    ) -> Result<WorkerInstanceId, carosign_core::ids::IdUnavailable> {
        self.inner.new_worker_instance_id()
    }
    fn new_evidence_id(&self) -> Result<EvidenceId, carosign_core::ids::IdUnavailable> {
        self.inner.new_evidence_id()
    }
    fn new_idempotency_key(&self) -> Result<IdempotencyKey, carosign_core::ids::IdUnavailable> {
        self.inner.new_idempotency_key()
    }
    fn new_connection_id(&self) -> Result<ConnectionId, carosign_core::ids::IdUnavailable> {
        self.inner.new_connection_id()
    }
}
