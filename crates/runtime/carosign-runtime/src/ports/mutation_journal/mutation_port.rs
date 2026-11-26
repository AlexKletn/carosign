use super::super::BoxFuture;
use super::DurableMarker;
use carosign_core::ids::OperationId;

/// The action with the storage that the engine asks for.
///
/// ⚠️ Runtime doesn't know the tables: it asks for a magazine, and the magazine knows how.
pub trait MutationPort: Send + Sync {
    /// Recording intent.
    fn persist_intent(&self, operation: OperationId) -> BoxFuture<'_, DurableMarker>;
    /// Secures the registry.
    fn commit_registry(&self, operation: OperationId) -> BoxFuture<'_, DurableMarker>;
    /// Sticks to the result of the reconciliation.
    fn commit_reconciliation(&self, operation: OperationId) -> BoxFuture<'_, DurableMarker>;
    /// He writes down "needs a reconciliation."
    fn mark_uncertain(&self, operation: OperationId) -> BoxFuture<'_, DurableMarker>;
    /// Closes the change.
    fn finalize(&self, operation: OperationId) -> BoxFuture<'_, DurableMarker>;
}
