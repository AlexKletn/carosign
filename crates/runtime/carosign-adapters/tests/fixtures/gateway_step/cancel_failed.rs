use carosign_core::ids::OperationId;
use carosign_runtime::ports::{CommandPlan, DispatchOutcome, ProviderExecutionPort};

pub(super) struct CancelFailed;

impl ProviderExecutionPort for CancelFailed {
    fn dispatch(
        &self,
        _: OperationId,
        _: CommandPlan,
    ) -> carosign_runtime::BoxFuture<'_, DispatchOutcome> {
        Box::pin(async { DispatchOutcome::NotSent })
    }
    fn request_cancel(&self, _: OperationId) -> carosign_runtime::BoxFuture<'_, bool> {
        Box::pin(async { false })
    }
}
