use super::*;
use std::task::Poll;

pub(super) struct GateBroker {
    pub(super) ready: AtomicBool,
}

impl TrustedCleanupBroker for GateBroker {
    fn quiesce<'a>(
        &'a self,
        scope: &'a CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        Box::pin(async move { Ok(scope.clone()) })
    }
    fn release<'a>(
        &'a self,
        scope: &'a CleanupScope,
        proof: QuiescedLease,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        assert_eq!(scope, proof.scope());
        Box::pin(std::future::poll_fn(move |_| {
            if self.ready.load(Ordering::SeqCst) {
                Poll::Ready(Ok(scope.clone()))
            } else {
                Poll::Pending
            }
        }))
    }
}
