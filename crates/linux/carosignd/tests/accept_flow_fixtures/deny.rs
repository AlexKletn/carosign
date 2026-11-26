use super::*;

pub(super) struct Deny;

impl PolicyAuthority for Deny {
    fn authorize(
        &self,
        _caller: &CallerContext,
        _scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Denied) })
    }
    fn revalidate(
        &self,
        _grant: &ValidatedGrant,
        _boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        Box::pin(async { Ok(()) })
    }
}
