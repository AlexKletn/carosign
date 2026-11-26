use crate::model::StorageError;

/// Refusal of steady step: With an unknown result commit, you cannot repeat the change until
/// the state is read from the repository.
#[derive(Debug)]
pub enum DurableRegistryFailure {
    BeforeCommit(StorageError),
    CommitUnknown(StorageError),
}
