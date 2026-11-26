use super::*;

pub(super) struct OwnerAuthority<'a> {
    pub(super) registry: &'a crate::operations::OperationRegistry,
    pub(super) operation: OperationId,
    pub(super) revoke: bool,
    pub(super) deny: bool,
}

impl carosign_runtime::ports::PolicyAuthority for OwnerAuthority<'_> {
    fn revalidate_caller<'a>(
        &'a self,
        _: &'a carosign_runtime::ports::CallerContext,
        grant: &'a carosign_core::authorization::ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        self.revalidate(grant, boundary)
    }
    fn authorize(
        &self,
        _: &carosign_runtime::ports::CallerContext,
        _: carosign_core::authorization::GrantScope,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<
            carosign_core::authorization::ValidatedGrant,
            carosign_runtime::ports::AuthorityRefusal,
        >,
    > {
        panic!("здесь выполняется только перепроверка")
    }
    fn revalidate(
        &self,
        _: &carosign_core::authorization::ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        assert_eq!(
            boundary,
            carosign_core::authorization::AuthorizationBoundary::BeforeDelivery
        );
        Box::pin(async move {
            if self.revoke {
                assert!(self.registry.revoke_verification(self.operation).is_ok());
            }
            if self.deny {
                Err(carosign_runtime::ports::AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }
}
