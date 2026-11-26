use crate::cleanup_owner::CleanupOwnerError;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EffectOwnerError {
    /// The previous step of the operation has not yet been transmitted.
    Duplicate,
    /// Limited queue is full.
    Limit,
    /// This result of the observer requires separate processing.
    UnsupportedControl,
    /// The cleaning result does not contain a step suitable for transmission.
    UnsupportedCleanup,
    ScheduledCleanup(crate::operations::CleanupFailure),
    /// The recording of the result does not contain a step for transmission.
    UnsupportedRecord,
    /// There is no expected effect.
    NotPending,
    /// The effect is already holding another performer.
    Busy,
    /// Performance may have started; automatic repetition is prohibited.
    Uncertain,
    /// Cleaning is not accepted by its owner.
    Cleanup(CleanupOwnerError),
}
