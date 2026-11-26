use carosign_core::ids::OperationId;
use carosign_runtime::ports::{BoxFuture, DurableMarker, MutationPort};

pub(super) struct UnusedMutation;

impl MutationPort for UnusedMutation {
    fn persist_intent(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        unreachable!()
    }
    fn commit_registry(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        unreachable!()
    }
    fn commit_reconciliation(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        unreachable!()
    }
    fn mark_uncertain(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        unreachable!()
    }
    fn finalize(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        unreachable!()
    }
}
