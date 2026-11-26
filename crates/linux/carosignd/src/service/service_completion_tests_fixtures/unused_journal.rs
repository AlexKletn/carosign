use super::*;

pub(super) struct UnusedJournal;

impl MutationPort for UnusedJournal {
    fn persist_intent(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        panic!("это проверка")
    }
    fn commit_registry(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        panic!("это проверка")
    }
    fn commit_reconciliation(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        panic!("это проверка")
    }
    fn mark_uncertain(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        panic!("это проверка")
    }
    fn finalize(&self, _: OperationId) -> BoxFuture<'_, DurableMarker> {
        panic!("терминал проверки только в памяти")
    }
}
