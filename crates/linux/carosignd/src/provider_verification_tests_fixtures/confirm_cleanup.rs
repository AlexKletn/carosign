pub(super) struct ConfirmCleanup;

impl crate::resources::TrustedCleanupBroker for ConfirmCleanup {
    fn quiesce<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>,
    > {
        Box::pin(async move { Ok(scope.clone()) })
    }
    fn release<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
        proof: crate::resources::QuiescedLease,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>,
    > {
        assert_eq!(proof.scope(), scope);
        Box::pin(async move { Ok(scope.clone()) })
    }
}
