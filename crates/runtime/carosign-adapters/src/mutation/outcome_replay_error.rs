use super::outcome_read_error::OutcomeReadError;

/// The error of repeating the events of the completed change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutcomeReplayError {
    /// Exodus or authority not confirmed.
    Outcome(OutcomeReadError),
    /// The image does not relate to this flow or operation.
    Snapshot(carosign_runtime::operation::events::SnapshotError),
}
