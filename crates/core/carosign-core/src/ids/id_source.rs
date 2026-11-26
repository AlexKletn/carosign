use super::CommandId;
use super::ConnectionId;
use super::EvidenceId;
use super::IdUnavailable;
use super::IdempotencyKey;
use super::LeaseId;
use super::MutationId;
use super::OperationId;
use super::WorkerInstanceId;

/// Waiting for `IdSource`: to generate 16 bytes without meaning.
///
/// ⚠️ It's ** trait only. "" The incarnation is created by the build and, on Linux, reads
/// random bytes at the kernel; c `core` He's not, and he can't be.
/// The different methods, because `OperationId`, `MutationId` The rest are created by
/// different legal owners (Document 01, N-02).
pub trait IdSource {
    /// New surgery.
    fn new_operation_id(&self) -> Result<OperationId, IdUnavailable>;
    /// A new change.
    fn new_mutation_id(&self) -> Result<MutationId, IdUnavailable>;
    /// A new host ISP command.
    fn new_command_id(&self) -> Result<CommandId, IdUnavailable>;
    /// A new provider process.
    fn new_worker_instance_id(&self) -> Result<WorkerInstanceId, IdUnavailable>;
    /// New resource allocations.
    fn new_lease_id(&self) -> Result<LeaseId, IdUnavailable>;
    /// New reference to proof.
    fn new_evidence_id(&self) -> Result<EvidenceId, IdUnavailable>;
    /// A new idempotence key issued by the **platform**, if the client did not give it, the
    /// client key comes in bytes and is verified separately.
    fn new_idempotency_key(&self) -> Result<IdempotencyKey, IdUnavailable>;
    fn new_connection_id(&self) -> Result<ConnectionId, IdUnavailable>;
}
