use carosign_core::ids::{MutationId, OperationId};

/// A trusted read port of the completed mutation outcome.
pub trait TerminalOutcomePort: Send + Sync {
    /// Reads a stable outcome, comparing the operation and the change.
    fn read(
        &self,
        operation: OperationId,
        mutation: MutationId,
    ) -> crate::ports::BoxFuture<'_, Result<carosign_engine::TerminalOutcome, ()>>;
}
