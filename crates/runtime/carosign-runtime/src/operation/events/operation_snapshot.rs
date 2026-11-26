use carosign_core::ids::OperationId;
use carosign_engine::{OperationPhase, TerminalOutcome};

/// Internal image; the right of observation is checked separately.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OperationSnapshot {
    pub operation: OperationId,
    /// Phase at the time of the photo.
    pub phase: OperationPhase,
    /// The last number of the event.
    pub last_sequence: u64,
    /// Completed test result, if available.
    pub terminal: Option<TerminalOutcome>,
}
