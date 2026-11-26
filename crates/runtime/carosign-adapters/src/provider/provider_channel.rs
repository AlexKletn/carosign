use super::provider_failure::ProviderFailure;
use carosign_core::ids::OperationId;
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_runtime::ports::DispatchOutcome;

/// What an adapter can do with a channel.
///
/// ⚠️ The trait is narrow on purpose: no sockets, no processes. `carosign-linux-ipc`, and in
/// checks, a stand.
pub trait ProviderChannel: Send + Sync {
    /// Sends a shot and says it came out.
    fn send(&self, frame: provider::ProviderFrame) -> Result<(), ProviderFailure>;
    /// One-time modifying shipment. The implementation is obliged to spend the ticket.
    fn send_mutation(
        &self,
        _frame: provider::ProviderFrame,
        _ticket: carosign_persistence::journal::ArmedMutationTicket,
        _context: carosign_runtime::ports::MutationArmRequest,
    ) -> DispatchOutcome {
        DispatchOutcome::Unknown
    }
    /// Continuation is transmitted only through the verified sending border.
    fn send_continuation(
        &self,
        _frame: provider::ProviderFrame,
        _ticket: carosign_persistence::journal::ArmedMutationTicket,
        _context: carosign_runtime::ports::MutationArmRequest,
        _boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> DispatchOutcome {
        DispatchOutcome::Unknown
    }
    /// He's asking for cancellation in a separate way.
    fn cancel(&self, operation: OperationId) -> bool;

    /// Requests cancellation by exact withholding issue and command ID.
    fn cancel_scoped(
        &self,
        _lease: &carosign_runtime::ports::ResourceLease,
        _command: carosign_core::ids::CommandId,
    ) -> DispatchOutcome {
        DispatchOutcome::NotSent
    }
}
