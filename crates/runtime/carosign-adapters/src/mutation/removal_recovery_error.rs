use super::storage_failure::StorageFailure;

/// Failure to restore deletion for independent verification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemovalRecoveryError {
    /// The record, area, or era has not been verified.
    Storage(StorageFailure),
    /// The record describes a different kind of change.
    WrongTransition,
    /// The engine rejected the restored context.
    InvalidState,
}
