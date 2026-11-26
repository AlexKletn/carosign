use super::ObservationFailure;

/// Failure to transfer deferred control without losing a step.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeferredControlFailure {
    Operation(ObservationFailure),
    NoControl,
    Effects(crate::effect_owner::EffectOwnerError),
}
