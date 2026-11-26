use super::*;

pub(super) struct Revoked;

impl PolicyAuthority for Revoked {
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        Allow.authorize(caller, scope)
    }

    fn revalidate(
        &self,
        _grant: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        assert_eq!(boundary, AuthorizationBoundary::BeforeDispatch);
        Box::pin(async { Err(AuthorityRefusal::Revoked) })
    }
}
