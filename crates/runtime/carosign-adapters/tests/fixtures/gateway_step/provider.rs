use carosign_core::ids::OperationId;
use carosign_runtime::ports::{CommandPlan, DispatchOutcome, ProviderExecutionPort};

pub(super) struct Provider(pub(super) std::sync::atomic::AtomicUsize);

impl ProviderExecutionPort for Provider {
    fn dispatch(
        &self,
        _operation: OperationId,
        _command: CommandPlan,
    ) -> carosign_runtime::BoxFuture<'_, DispatchOutcome> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { DispatchOutcome::Accepted })
    }
    fn request_cancel(&self, _operation: OperationId) -> carosign_runtime::BoxFuture<'_, bool> {
        Box::pin(async { true })
    }
}
