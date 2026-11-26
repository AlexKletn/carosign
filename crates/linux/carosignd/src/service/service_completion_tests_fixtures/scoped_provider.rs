use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct ScopedProvider(pub(super) AtomicUsize);

impl ProviderExecutionPort for ScopedProvider {
    fn dispatch(&self, _: OperationId, _: CommandPlan) -> BoxFuture<'_, DispatchOutcome> {
        Box::pin(async { DispatchOutcome::NotSent })
    }
    fn request_cancel(&self, _: OperationId) -> BoxFuture<'_, bool> {
        panic!("нельзя обходить проверку области")
    }
    fn request_cancel_scoped<'a>(
        &'a self,
        lease: &'a ResourceLease,
        command: CommandId,
    ) -> BoxFuture<'a, DispatchOutcome> {
        assert_eq!(
            lease.owner().operation,
            OperationId::from_bytes([3; 16]).unwrap()
        );
        assert_eq!(command, CommandId::from_bytes([2; 16]).unwrap());
        self.0.fetch_add(1, Ordering::SeqCst);
        // Rejection does not prove that the command did not reach.
        Box::pin(async { DispatchOutcome::Unknown })
    }
}
