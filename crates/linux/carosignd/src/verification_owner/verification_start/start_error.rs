use super::{DriveError, EffectOwnerError};

#[derive(Debug)]
pub(crate) enum StartError {
    Operation(crate::operations::ObservationFailure),
    Handoff(EffectOwnerError),
    Drive(DriveError),
    Preparation(crate::operations::VerificationPreparationFailure),
    OutstandingEffects,
    Send(crate::provider_verification::VerificationError),
    Finished,
}
