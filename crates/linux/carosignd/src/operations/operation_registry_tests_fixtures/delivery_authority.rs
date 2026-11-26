use super::*;
use carosign_core::authorization::{AuthorizationBoundary, GrantScope};
use carosign_runtime::ports::{AuthorityRefusal, BoxFuture, PolicyAuthority};

pub(super) struct Authority(pub(super) bool);

impl PolicyAuthority for Authority {
    fn revalidate_caller<'a>(
        &'a self,
        _: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        self.revalidate(grant, boundary)
    }
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        assert_eq!(boundary, AuthorizationBoundary::BeforeDelivery);
        Box::pin(async move {
            if self.0 {
                Ok(())
            } else {
                Err(AuthorityRefusal::Unavailable)
            }
        })
    }
}
