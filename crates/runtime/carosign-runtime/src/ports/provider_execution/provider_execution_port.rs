use super::super::BoxFuture;
use super::super::MutationArmRequest;
use super::super::ResourceLease;
use super::CommandPlan;
use super::DispatchOutcome;
use carosign_core::ids::OperationId;

/// A wire to the provider.
///
/// The port sends a command and obtains a receipt. The receipt does not establish execution.
pub trait ProviderExecutionPort: Send + Sync {
    /// Sends a command.
    fn dispatch(
        &self,
        operation: OperationId,
        command: CommandPlan,
    ) -> BoxFuture<'_, DispatchOutcome>;

    /// Accepts a one-time ticket to change. Failure to implement does not negate sustainable
    /// weaponry and leaves the outcome uncertain.
    fn dispatch_mutation(
        &self,
        _ticket: carosign_persistence::journal::ArmedMutationTicket,
        _context: MutationArmRequest,
    ) -> BoxFuture<'_, DispatchOutcome> {
        Box::pin(async { DispatchOutcome::Unknown })
    }

    /// He's asking for cancellation in a separate way.
    fn request_cancel(&self, operation: OperationId) -> BoxFuture<'_, bool>;

    /// Requests cancellation on withheld issuance and one-time ID.
    /// The form of extradition itself does not replace the verification of its owner by an
    /// broker.
    fn request_cancel_scoped<'a>(
        &'a self,
        _lease: &'a ResourceLease,
        _command: carosign_core::ids::CommandId,
    ) -> BoxFuture<'a, DispatchOutcome> {
        Box::pin(async { DispatchOutcome::NotSent })
    }
}
