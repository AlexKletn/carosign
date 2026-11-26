use carosign_core::ids::CommandId;
use carosign_core::ids::MutationId;
use carosign_core::ids::OperationId;

/// Checking permission to ship.
///
/// ⚠️ Implemented by the repository: the mediator asks, not invents.
pub trait DispatchAuthorization {
    /// Is there a stable record? `DispatchArmed` for this threesome.
    fn is_armed(&self, operation: OperationId, mutation: MutationId, command: CommandId) -> bool;
}
