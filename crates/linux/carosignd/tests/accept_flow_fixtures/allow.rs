use super::*;

pub(super) struct Allow;

impl PolicyAuthority for Allow {
    fn authorize(
        &self,
        _caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        let grant = ValidatedGrant::issue_for_port(
            GrantId::from_bytes([1u8; 16]).expect("валид"),
            AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            scope,
            BootId::from_bytes([3u8; 16]).expect("валид"),
            1_000,
        );
        Box::pin(async move { Ok(grant) })
    }
    fn revalidate(
        &self,
        _grant: &ValidatedGrant,
        _boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        Box::pin(async { Ok(()) })
    }
}
