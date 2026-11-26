use super::*;

pub(super) struct MockCleanupBroker {
    pub(super) quiesce: MockReply,
    pub(super) release: MockReply,
    pub(super) quiesce_calls: AtomicUsize,
    pub(super) release_calls: AtomicUsize,
}

impl MockCleanupBroker {
    pub(super) fn new(quiesce: MockReply, release: MockReply) -> Self {
        Self {
            quiesce,
            release,
            quiesce_calls: AtomicUsize::new(0),
            release_calls: AtomicUsize::new(0),
        }
    }
}

impl TrustedCleanupBroker for MockCleanupBroker {
    fn quiesce<'a>(
        &'a self,
        scope: &'a CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        self.quiesce_calls.fetch_add(1, Ordering::SeqCst);
        mock_reply(self.quiesce, scope)
    }
    fn release<'a>(
        &'a self,
        scope: &'a CleanupScope,
        proof: QuiescedLease,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        assert_eq!(scope, proof.scope());
        self.release_calls.fetch_add(1, Ordering::SeqCst);
        mock_reply(self.release, scope)
    }
}
