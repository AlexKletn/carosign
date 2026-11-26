use carosign_core::ids::OperationId;
use carosign_runtime::ports::{DurableMarker, MutationPort};

pub(super) struct Journal(pub(super) std::sync::atomic::AtomicUsize);

impl MutationPort for Journal {
    fn persist_intent(
        &self,
        _operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn commit_registry(
        &self,
        _operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn commit_reconciliation(
        &self,
        _operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn mark_uncertain(
        &self,
        _operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn finalize(&self, _operation: OperationId) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
}
