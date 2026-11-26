use super::{
    BoxFuture, CommandPlan, DispatchOutcome, DurableMarker, MutationPort, OperationId,
    ProviderExecutionPort,
};

pub(super) struct VerificationOnly;

impl ProviderExecutionPort for VerificationOnly {
    fn dispatch(&self, _: OperationId, _: CommandPlan) -> BoxFuture<'_, DispatchOutcome> {
        Box::pin(async { DispatchOutcome::NotSent })
    }
    fn request_cancel(&self, _: OperationId) -> BoxFuture<'_, bool> {
        Box::pin(async { false })
    }
}

impl MutationPort for VerificationOnly {
    fn persist_intent(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn commit_registry(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn commit_reconciliation(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn mark_uncertain(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
    fn finalize(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        Box::pin(async { DurableMarker { durable: false } })
    }
}
