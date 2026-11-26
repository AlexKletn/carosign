use carosign_core::ids::OperationId;
use carosign_runtime::ports::{DurableMarker, MutationPort};

pub(super) struct FailingJournal;

impl MutationPort for FailingJournal {
    fn persist_intent(&self, _: OperationId) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn commit_registry(&self, _: OperationId) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn commit_reconciliation(
        &self,
        _: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn mark_uncertain(&self, _: OperationId) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn finalize(&self, _: OperationId) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
}
