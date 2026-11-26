use carosign_core::ids::{MutationId, OperationId};

/// Session-tested connection of the registration response to the active command.
///
/// It's not a proof of a device's status or a registry right. It can only be created by a
/// session check.
#[derive(Debug, PartialEq)]
pub struct EnrollmentOutcome {
    pub(super) event: carosign_provider_sdk::events::Event,
    pub(super) operation: OperationId,
    pub(super) mutation: MutationId,
    pub(super) planned: carosign_core::enrollment::EnrollmentRef,
}

impl EnrollmentOutcome {
    /// Initial statement of the provider, together with references and context.
    pub fn event(&self) -> &carosign_provider_sdk::events::Event {
        &self.event
    }
    /// The operation with which the answer is verified.
    pub fn operation(&self) -> OperationId {
        self.operation
    }
    /// The change with which the answer is verified.
    pub fn mutation(&self) -> MutationId {
        self.mutation
    }
    /// Planned registration with which the echo is verified.
    pub fn planned(&self) -> carosign_core::enrollment::EnrollmentRef {
        self.planned
    }
}
