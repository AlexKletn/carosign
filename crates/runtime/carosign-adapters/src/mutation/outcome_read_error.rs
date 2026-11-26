use super::storage_failure::StorageFailure;

/// Refusal to read a sustainable result for the observer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutcomeReadError {
    /// The source of the policy did not allow extradition.
    Authority(carosign_runtime::ports::AuthorityRefusal),
    /// A sustainable outcome is not available or agreed upon.
    Storage(StorageFailure),
}
