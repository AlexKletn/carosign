use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::ids::{AuthorityId, BootId, GrantId};
use carosign_runtime::ports::{AuthorityRefusal, BoxFuture, CallerContext, PolicyAuthority};

pub(super) struct Policy {
    pub(super) scope: GrantScope,
    pub(super) revoked: bool,
}

impl PolicyAuthority for Policy {
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        let grant = ValidatedGrant::issue_for_port(
            GrantId::from_bytes([1; 16]).unwrap(),
            AuthorityId::from_bytes([2; 16]).unwrap(),
            self.scope.clone(),
            BootId::from_bytes([3; 16]).unwrap(),
            100,
        );
        Box::pin(async move { Ok(grant) })
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        assert_eq!(boundary, AuthorizationBoundary::BeforeDelivery);
        let revoked = self.revoked;
        Box::pin(async move {
            if revoked {
                Err(AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }
    fn revalidate_caller<'a>(
        &'a self,
        caller: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        if caller.uid() != 1000 {
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        }
        self.revalidate(grant, boundary)
    }
}
