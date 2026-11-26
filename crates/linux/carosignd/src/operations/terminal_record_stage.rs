/// The fixation of a negative outcome exists only in the memory of this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TerminalRecordStage {
    NotStarted,
    Recorded {
        effect: carosign_engine::PlannedEffect,
        outcome: carosign_engine::TerminalOutcome,
    },
}
