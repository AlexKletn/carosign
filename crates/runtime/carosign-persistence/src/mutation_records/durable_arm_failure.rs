use crate::model::StorageError;

/// Sustainablely arming exactly one shipment and only then issues a ticket.
///
/// Authority, live resource allocation and ownership of the effect to the current actor is
/// checked by the caller. Ambiguous failure to fix does not issue a ticket; further recovery
/// reads the journal, but does not reproduce the send.
#[derive(Debug, PartialEq, Eq)]
pub enum DurableArmFailure {
    /// This call has not reached the point of fixation; the old arm may already be recorded.
    BeforeCommit(StorageError),
    /// The fixation has begun, but its sustainable outcome has not been confirmed.
    CommitUnknown(StorageError),
}

impl From<StorageError> for DurableArmFailure {
    fn from(error: StorageError) -> Self {
        Self::BeforeCommit(error)
    }
}
