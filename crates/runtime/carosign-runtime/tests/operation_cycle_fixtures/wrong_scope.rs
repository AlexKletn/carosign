use super::*;

pub(super) struct WrongScope;

impl PolicyAuthority for WrongScope {
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        let other = SubjectRef {
            authority: AuthorityId::from_bytes([2; 16]).unwrap(),
            subject: SubjectId::from_bytes([99; 16]).unwrap(),
        };
        Box::pin(async move {
            Ok(ValidatedGrant::issue_for_port(
                GrantId::from_bytes([1; 16]).unwrap(),
                AuthorityId::from_bytes([2; 16]).unwrap(),
                verify_scope(other),
                BootId::from_bytes([3; 16]).unwrap(),
                1_000,
            ))
        })
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        _: carosign_core::authorization::AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        Box::pin(async { Ok(()) })
    }
}
