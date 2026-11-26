use super::*;

pub(super) struct UnboundAuthority;

impl carosign_runtime::ports::PolicyAuthority for UnboundAuthority {
    fn authorize(
        &self,
        _: &CallerContext,
        _: carosign_core::authorization::GrantScope,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<ValidatedGrant, carosign_runtime::ports::AuthorityRefusal>,
    > {
        unreachable!()
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        _: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        Box::pin(async { Ok(()) })
    }
}
