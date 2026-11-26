use crate::operation::events::Progress;
use carosign_core::ids::OperationId;

/// The contents of the move after the session check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressObservation {
    /// The operation to which the event relates.
    pub operation: OperationId,
    /// The worker's event number.
    pub provider_sequence: u64,
    /// A clue for the observer.
    pub progress: Progress,
}
