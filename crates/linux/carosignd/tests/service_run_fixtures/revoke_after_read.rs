use super::*;

pub(super) struct RevokeAfterRead(pub(super) std::sync::atomic::AtomicUsize);

impl PolicyAuthority for RevokeAfterRead {
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Denied) })
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        _: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        let count = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async move {
            if count == 0 {
                Ok(())
            } else {
                Err(AuthorityRefusal::Revoked)
            }
        })
    }
}
