/// Why an actor can't hold back an answer until it's sustainable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationError {
    /// The answer does not apply to the current plan or phase.
    ContextMismatch,
    /// The previous sighting is still awaiting recording.
    AlreadyPending,
    /// There is no observation or has already been transmitted to the engine.
    NoObservation,
    /// A match requires a separate verification of the evidence.
    EvidenceRequired,
    /// The cause of the provider's error is not yet carried forward by the engine input.
    ProviderDetailUnavailable,
    /// The usual queue is filled; the watch remains with the actor.
    MailboxFull,
}
