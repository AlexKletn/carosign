use super::*;

pub(super) struct Ids(pub(super) std::sync::atomic::AtomicU64);

impl IdSource for Ids {
    fn new_operation_id(&self) -> Result<OperationId, carosign_core::ids::IdUnavailable> {
        let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let mut bytes = [0u8; 16];
        bytes[15] = n as u8;
        Ok(OperationId::from_bytes(bytes).expect("ненулевой"))
    }
    fn new_mutation_id(&self) -> Result<MutationId, carosign_core::ids::IdUnavailable> {
        Ok(MutationId::from_bytes([1u8; 16]).expect("валид"))
    }
    fn new_command_id(&self) -> Result<CommandId, carosign_core::ids::IdUnavailable> {
        Ok(CommandId::from_bytes([2u8; 16]).expect("валид"))
    }
    fn new_worker_instance_id(
        &self,
    ) -> Result<WorkerInstanceId, carosign_core::ids::IdUnavailable> {
        Ok(WorkerInstanceId::from_bytes([3u8; 16]).expect("валид"))
    }
    fn new_lease_id(&self) -> Result<LeaseId, carosign_core::ids::IdUnavailable> {
        Ok(LeaseId::from_bytes([4u8; 16]).expect("валид"))
    }
    fn new_evidence_id(&self) -> Result<EvidenceId, carosign_core::ids::IdUnavailable> {
        Ok(EvidenceId::from_bytes([5u8; 16]).expect("валид"))
    }
    fn new_idempotency_key(&self) -> Result<IdempotencyKey, carosign_core::ids::IdUnavailable> {
        Ok(IdempotencyKey::from_bytes([6u8; 16]).expect("валид"))
    }
    fn new_connection_id(&self) -> Result<ConnectionId, carosign_core::ids::IdUnavailable> {
        Ok(ConnectionId::from_bytes([8u8; 16]).expect("валид"))
    }
}
