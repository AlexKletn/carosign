/// Why can't an indefinite cleaning be passed on to an actor?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupFailure {
    /// The operation is missing or is already being processed.
    OperationUnavailable,
    /// This isn't a checkup operation.
    WrongOperationKind,
    /// The operation has no withholding rendition.
    NoRetainedLease,
    /// Issuance does not belong to the operation and current binding of the device.
    LeaseContextMismatch,
    /// The transferred effect is not a current cleaning request.
    EffectMismatch,
    /// First, you need to process previously received messages.
    AwaitingActor,
    /// The queue of the actor did not accept the observation.
    MailboxFull,
    /// The cleaning or confirmation has already been completed.
    AlreadyFinished,
}
