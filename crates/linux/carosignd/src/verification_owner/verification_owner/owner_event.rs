use super::PumpEvent;

/// A session event or a sign of a delayed step.
#[derive(Debug)]
pub(crate) enum OwnerEvent {
    /// You need to pass the step before the next reading.
    StepReady,
    /// The event without step remains with the caller.
    Other(PumpEvent),
}
