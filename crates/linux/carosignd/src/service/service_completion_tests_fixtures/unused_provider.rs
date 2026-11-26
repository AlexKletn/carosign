use super::*;

pub(super) struct UnusedProvider;

impl ProviderExecutionPort for UnusedProvider {
    fn dispatch(&self, _: OperationId, _: CommandPlan) -> BoxFuture<'_, DispatchOutcome> {
        panic!("служба ждёт привязки сеанса")
    }
    fn request_cancel(&self, _: OperationId) -> BoxFuture<'_, bool> {
        panic!("отмена не запрашивалась")
    }
}
