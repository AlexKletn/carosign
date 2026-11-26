/// Why was the surveillance not transmitted?
#[derive(Debug, PartialEq, Eq)]
pub enum ObservationFailure {
    /// There's no saved operation with this number.
    UnknownOperation,
    /// The actor is already being handled by another owner.
    Busy,
    /// The actor has rejected the context or is already holding back another observation.
    Rejected(carosign_runtime::operation::actor::ObservationError),
    /// The context of the session does not correspond to the actor or the expected effect.
    ContextMismatch,
    /// The usual turn of the actor is filled; the event can be repeated.
    MailboxFull,
    /// The actor has not yet processed the confirmation or other message.
    AwaitingActor,
    /// The operation has expired or the clock belongs to another load.
    DeadlineExpired,
}
