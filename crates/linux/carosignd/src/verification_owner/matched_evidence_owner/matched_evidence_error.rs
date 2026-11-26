use super::EffectOwnerError;

#[derive(Debug)]
pub(crate) enum MatchedEvidenceError {
    Operation(crate::operations::ObservationFailure),
    Handoff(EffectOwnerError),
    InvalidStage,
}
