use super::{EffectOwnerError, StepPublishError};

#[derive(Debug)]
pub(crate) enum DriveError {
    Publish(StepPublishError),
    Effect(EffectOwnerError),
    Operation(crate::operations::ObservationFailure),
}
