pub(super) struct LegacyAuthority;

impl carosign_runtime::ports::PolicyAuthority for LegacyAuthority {
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
        Box::pin(async { Err(carosign_runtime::ports::AuthorityRefusal::Unavailable) })
    }
    fn revalidate(
        &self,
        _: &carosign_core::authorization::ValidatedGrant,
        _: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        Box::pin(async { Ok(()) })
    }
}
