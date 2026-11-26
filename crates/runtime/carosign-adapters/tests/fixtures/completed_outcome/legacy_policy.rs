use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_runtime::ports::{AuthorityRefusal, BoxFuture, CallerContext, PolicyAuthority};

use super::policy::Policy;

pub(super) struct LegacyPolicy(pub(super) Policy);

impl PolicyAuthority for LegacyPolicy {
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
