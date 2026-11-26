use super::VerificationError;

/// Error of passing the event to the actor; no option confirms cleaning.
#[derive(Debug)]
pub enum ActorVerificationError {
    /// Error of exchange with the provider.
    Exchange(VerificationError),
    /// The actor belongs to a different context or does not expect the desired effect.
    ContextMismatch,
    /// The message could not be put in line.
    MailboxFull,
    /// The actor did not accept the observation verified by the session.
    Observation(carosign_runtime::operation::actor::ObservationError),
}
