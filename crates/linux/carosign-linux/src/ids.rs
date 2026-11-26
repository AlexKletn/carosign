//! Source of identifiers: 16 random bytes at the kernel.
//!
//! ⚠️ Rule of document 03: `IdSource` - ** trait only in the domain, and the incarnation
//! lives where you can ask randomness. You can't ask "anything plus time": two runs in one
//! second would give one identifier, and other people's operations would mix.
//!
//! ⚠️ Zero is rejected: [`IdSource`] requires non-zero values, and this is tested, not
//! assumed.

use carosign_core::ids::{
    CommandId, ConnectionId, EvidenceId, IdSource, IdUnavailable, IdempotencyKey, LeaseId,
    MutationId, OperationId, WorkerInstanceId,
};

use crate::sys;

/// Source `getrandom(2)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct KernelIds;

impl KernelIds {
    /// Creates a stateless source backed by kernel randomness.
    pub fn new() -> Self {
        Self
    }

    /// Sixteen random bytes, not all zero.
    ///
    /// ⚠️ `unsafe` dwells `sys`: There is only the rule "zero - failure, and try again."
    pub(crate) fn bytes(&self) -> Result<[u8; 16], IdUnavailable> {
        nonzero_bytes(sys::random_16)
    }
}

fn nonzero_bytes(
    mut random: impl FnMut() -> Result<[u8; 16], sys::SysError>,
) -> Result<[u8; 16], IdUnavailable> {
    for _ in 0..8 {
        match random() {
            Ok(bytes) if bytes.iter().any(|byte| *byte != 0) => return Ok(bytes),
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    Err(IdUnavailable)
}

impl IdSource for KernelIds {
    fn new_operation_id(&self) -> Result<OperationId, IdUnavailable> {
        OperationId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_mutation_id(&self) -> Result<MutationId, IdUnavailable> {
        MutationId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_command_id(&self) -> Result<CommandId, IdUnavailable> {
        CommandId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_worker_instance_id(&self) -> Result<WorkerInstanceId, IdUnavailable> {
        WorkerInstanceId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_lease_id(&self) -> Result<LeaseId, IdUnavailable> {
        LeaseId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_evidence_id(&self) -> Result<EvidenceId, IdUnavailable> {
        EvidenceId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_idempotency_key(&self) -> Result<IdempotencyKey, IdUnavailable> {
        IdempotencyKey::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }

    fn new_connection_id(&self) -> Result<ConnectionId, IdUnavailable> {
        ConnectionId::from_bytes(self.bytes()?).map_err(|_| IdUnavailable)
    }
}

/// Convenient test: is randomness free?
pub fn available() -> bool {
    sys::random_available()
}

#[path = "ids.tests.rs"]
#[cfg(test)]
mod tests;
