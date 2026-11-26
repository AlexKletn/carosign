use super::{EffectOwnerError, PumpError};

#[derive(Debug)]
pub(crate) enum OwnerError {
    /// First, you need to transfer the already held step.
    Pending,
    /// The exchange did not take place or the actor did not accept the observation.
    Pump(PumpError),
    /// The owner of the effects has not taken the step; it remains held.
    Handoff(EffectOwnerError),
    /// There's nothing to pass.
    NoStep,
}
