use super::*;

#[derive(Default)]
pub(super) struct FailedTransport {
    pub(super) sent: std::sync::atomic::AtomicUsize,
    pub(super) cancelled: std::sync::atomic::AtomicUsize,
}

impl ProviderExecutionPort for FailedTransport {
    fn dispatch(&self, _: OperationId, _: CommandPlan) -> BoxFuture<'_, DispatchOutcome> {
        self.sent.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { DispatchOutcome::Failed })
    }
    fn request_cancel(&self, _: OperationId) -> BoxFuture<'_, bool> {
        self.cancelled
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { true })
    }
}
