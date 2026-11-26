use super::*;

pub(super) struct UnavailableAuthority;

impl PolicyAuthority for UnavailableAuthority {
    fn authorize(
        &self,
        _caller: &CallerContext,
        _scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }
    fn revalidate(
        &self,
        _grant: &ValidatedGrant,
        _boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }
}
