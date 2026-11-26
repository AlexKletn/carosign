/// The result of a limited run of a negative check.
#[derive(Debug)]
pub(crate) enum DriveOutcome {
    /// The terminal step is saved; eligibility and dispatch are checked separately.
    TerminalPending,
    /// Another performer is needed for the step; the effects remain withheld.
    ExternalEffect,
    /// Step limits are reached; the state remains held.
    StepLimit,
}
