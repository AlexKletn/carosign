use carosign_core::ids::{
    CommandId, ConnectionId, EvidenceId, IdSource, IdUnavailable, IdempotencyKey, LeaseId,
    MutationId, OperationId, WorkerInstanceId,
};

pub(super) struct CancelIds;

impl IdSource for CancelIds {
    fn new_operation_id(&self) -> Result<OperationId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_mutation_id(&self) -> Result<MutationId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_command_id(&self) -> Result<CommandId, IdUnavailable> {
        Ok(CommandId::from_bytes([22; 16]).unwrap())
    }
    fn new_worker_instance_id(&self) -> Result<WorkerInstanceId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_lease_id(&self) -> Result<LeaseId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_evidence_id(&self) -> Result<EvidenceId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_idempotency_key(&self) -> Result<IdempotencyKey, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_connection_id(&self) -> Result<ConnectionId, IdUnavailable> {
        Err(IdUnavailable)
    }
}
