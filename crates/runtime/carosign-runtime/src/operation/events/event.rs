use super::EventKind;
use carosign_engine::OperationPhase;

/// Event.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Event {
    pub sequence: u64,
    /// Phase at the time of the event.
    pub phase: OperationPhase,
    pub kind: EventKind,
}
