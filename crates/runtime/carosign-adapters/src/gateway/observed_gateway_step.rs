use super::gateway_step::GatewayStep;

/// The result of the step and independent notification of the status of observers.
#[derive(Debug)]
pub struct ObservedGatewayStep {
    /// Performing the effects of an actor.
    pub step: GatewayStep,
    /// Close connections or event flow error.
    pub notification: Result<Vec<u32>, carosign_runtime::operation::events::EventError>,
}
