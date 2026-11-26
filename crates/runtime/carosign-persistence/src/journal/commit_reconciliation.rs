use carosign_core::{enrollment::EnrollmentRef, ids::MutationId};

/// The result of the reconciliation that needs to be fixed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommitReconciliation {
    pub mutation: MutationId,
    /// Registry action (decision B-01).
    pub action: carosign_engine::RegistryAction,
    /// The record to be used (for the purpose of `ApplyDesiredState`).
    pub enrollment: Option<EnrollmentRef>,
}
