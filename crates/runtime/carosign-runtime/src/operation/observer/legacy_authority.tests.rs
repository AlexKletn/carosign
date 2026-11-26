use super::{
    AuthorityRefusal, AuthorizationBoundary, BoxFuture, CallerContext, GrantScope, Policy,
    PolicyAuthority, ValidatedGrant,
};

pub(super) struct Legacy(pub(super) Policy);
impl PolicyAuthority for Legacy {
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        self.0.authorize(caller, scope)
    }

    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        self.0.revalidate(grant, boundary)
    }
}
