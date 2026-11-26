use super::ProgressObservation;

/// The result of the transfer of the event to the actor of the operation.
#[derive(Debug)]
pub enum ActorVerificationMessage {
    /// Confirmation of sending awaits processing by the actor.
    Accepted,
    /// The provider refused to accept the command; the reason remains with the caller.
    Rejected(carosign_protocol::carosign::common::v1::Error),
    /// Proven operation.
    Progress(ProgressObservation),
    /// The negative result is placed in the actor’s queue.
    OutcomeQueued,
    /// The approval is withheld by the actor and requires additional processing.
    OutcomeHeld(carosign_runtime::operation::actor::ObservationError),
}
