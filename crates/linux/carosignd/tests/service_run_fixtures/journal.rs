use super::*;

pub(super) struct Journal;

impl MutationPort for Journal {
    fn persist_intent(&self, _o: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn commit_registry(&self, _o: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn commit_reconciliation(&self, _o: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn mark_uncertain(&self, _o: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
    fn finalize(&self, _o: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: true } })
    }
}
