use super::ObservationFailure;

pub(crate) enum VerificationBindingFailure {
    Operation(ObservationFailure),
    Handoff(crate::effect_owner::EffectOwnerError),
}
