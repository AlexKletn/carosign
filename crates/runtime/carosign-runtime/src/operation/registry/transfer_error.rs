use super::RegistryError;
use crate::recovery::blocked_scope::RecoveryError;

/// Refusal to transfer the operation for reconciliation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransferError {
    /// The operation is not available or is already in the hands of another contractor.
    Registry(RegistryError),
    /// The coordinator did not accept the actor; he remained on the roster.
    Recovery(RecoveryError),
}
