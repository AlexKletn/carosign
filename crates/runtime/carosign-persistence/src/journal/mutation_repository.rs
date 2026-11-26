use carosign_core::ids::MutationId;

use crate::model::{MutationClosure, MutationRecord, StorageError};

use super::{
    ArmMutation, ArmedMutationTicket, CommitEnrollment, CommitReconciliation, MutationObservation,
    PrepareMutation, PreparedMutation, RegistryCommit,
};

/// Change log.
///
/// ⚠️ It's the only legal way to change tables. `MetadataStore` Runtime does not: otherwise
/// the magazine would be bypassed by one line.
pub trait MutationRepository: Send + Sync {
    /// Records the intent and lock of an area with a single transaction.
    fn prepare(&self, input: PrepareMutation) -> Result<PreparedMutation, StorageError>;

    /// Records the permission to send and the queue - in one transaction.
    fn arm(&self, input: ArmMutation) -> Result<ArmedMutationTicket, StorageError>;

    /// Records the ISP's surveillance.
    fn record_observation(&self, input: MutationObservation) -> Result<(), StorageError>;

    /// The registry and the log phase are fixed in a single transaction.
    fn commit_registry(&self, input: CommitEnrollment) -> Result<RegistryCommit, StorageError>;

    /// Consolidates the result of the reconciliation (decision B-01).
    fn commit_reconciliation(
        &self,
        input: CommitReconciliation,
    ) -> Result<RegistryCommit, StorageError>;

    /// Closes the change and frees up the area.
    ///
    /// ⚠️ Only when`MutationClosure`], which is indeed liberating.
    fn finalize(&self, mutation: MutationId, closure: MutationClosure) -> Result<(), StorageError>;

    /// Marks the mutation requiring reconciliation and **holds* lock.
    fn mark_uncertain(&self, mutation: MutationId) -> Result<(), StorageError>;

    /// Returns records that are not closed.
    fn scan_incomplete(&self) -> Result<Vec<MutationRecord>, StorageError>;
}
