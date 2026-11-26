use super::ProgressObservation;

/// The event of the channel owner and the saved operation.
#[derive(Debug)]
pub enum PumpEvent {
    /// Confirmation is queued and the actor has taken one step.
    /// The step can handle an earlier control message; the effects remain with the caller.
    Accepted(carosign_runtime::operation::actor::Step),
    /// The worker refused admission.
    Rejected(carosign_protocol::carosign::common::v1::Error),
    /// Proven operation.
    Progress(ProgressObservation),
    /// The result is transferred to the actor, the effects remain with the caller.
    Outcome(crate::operations::ObservationProgress),
}
