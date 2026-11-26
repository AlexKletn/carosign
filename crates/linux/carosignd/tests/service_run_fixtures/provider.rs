use super::*;

pub(super) struct Provider(pub(super) std::sync::atomic::AtomicUsize);

impl ProviderExecutionPort for Provider {
    fn dispatch(
        &self,
        _operation: OperationId,
        _command: CommandPlan,
    ) -> BoxFuture<'_, DispatchOutcome> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { DispatchOutcome::Accepted })
    }
    fn request_cancel(&self, _operation: OperationId) -> BoxFuture<'_, bool> {
        Box::pin(async { true })
    }
}
