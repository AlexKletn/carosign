use super::{DriveOutcome, PumpEvent};

/// The result of one limited reception and processing of the saved step.
#[derive(Debug)]
pub(crate) enum AdvanceOutcome {
    /// The step is accepted by the performer and its available effects are processed.
    Driven(DriveOutcome),
    /// The event requires a separate decision of the owner of the composition.
    Other(PumpEvent),
}
