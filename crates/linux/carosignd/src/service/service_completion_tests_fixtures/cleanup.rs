use super::*;

pub(super) struct Cleanup;

impl crate::resources::TrustedCleanupBroker for Cleanup {
    fn quiesce<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
    ) -> BoxFuture<'a, Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>>
    {
        Box::pin(async move { Ok(scope.clone()) })
    }
    fn release<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
        proof: crate::resources::QuiescedLease,
    ) -> BoxFuture<'a, Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>>
    {
        assert_eq!(proof.scope(), scope);
        Box::pin(async move { Ok(scope.clone()) })
    }
}
