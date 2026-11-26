use super::*;

pub(super) struct Ids;

impl IdSource for Ids {
    fn new_operation_id(&self) -> Result<OperationId, IdUnavailable> {
        Ok(OperationId::from_bytes([3; 16]).unwrap())
    }
    fn new_mutation_id(&self) -> Result<MutationId, IdUnavailable> {
        Ok(MutationId::from_bytes([4; 16]).unwrap())
    }
    fn new_command_id(&self) -> Result<CommandId, IdUnavailable> {
        Ok(CommandId::from_bytes([2; 16]).unwrap())
    }
    fn new_worker_instance_id(&self) -> Result<WorkerInstanceId, IdUnavailable> {
        Ok(WorkerInstanceId::from_bytes([1; 16]).unwrap())
    }
    fn new_lease_id(&self) -> Result<LeaseId, IdUnavailable> {
        Ok(LeaseId::from_bytes([10; 16]).unwrap())
    }
    fn new_evidence_id(&self) -> Result<EvidenceId, IdUnavailable> {
        Ok(EvidenceId::from_bytes([11; 16]).unwrap())
    }
    fn new_idempotency_key(&self) -> Result<IdempotencyKey, IdUnavailable> {
        Ok(IdempotencyKey::from_bytes([12; 16]).unwrap())
    }
    fn new_connection_id(&self) -> Result<ConnectionId, IdUnavailable> {
        Ok(ConnectionId::from_bytes([13; 16]).unwrap())
    }
}
