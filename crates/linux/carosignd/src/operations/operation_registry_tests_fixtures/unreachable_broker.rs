pub(super) struct UnreachableBroker;

impl crate::resources::TrustedCleanupBroker for UnreachableBroker {
    fn quiesce<'a>(
        &'a self,
        _: &'a crate::resources::CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>,
    > {
        panic!("при немедленном сроке посредник не вызывается")
    }
    fn release<'a>(
        &'a self,
        _: &'a crate::resources::CleanupScope,
        _: crate::resources::QuiescedLease,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>,
    > {
        panic!("при немедленном сроке посредник не вызывается")
    }
}
