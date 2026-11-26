use super::super::BoxFuture;
use super::super::CallerContext;
use super::AuthorityRefusal;
use carosign_core::authorization::AuthorizationBoundary;
use carosign_core::authorization::GrantScope;
use carosign_core::authorization::ValidatedGrant;
use carosign_core::identity::SubjectRef;

/// Source of authority.
pub trait PolicyAuthority: Send + Sync {
    /// A source without such a binding fails; a caller's UID and a client's arbitrary field
    /// are not a substitute.
    fn subject_uid(&self, _subject: SubjectRef) -> Result<u32, AuthorityRefusal> {
        Err(AuthorityRefusal::Unavailable)
    }
    /// Issues permission based on the original request and the authorized subject.
    fn authorize_verification<'a>(
        &'a self,
        _caller: &'a CallerContext,
        _intent: &'a crate::authority::verification::VerificationIntent,
        _subject: SubjectRef,
    ) -> BoxFuture<'a, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }
    /// Issues a validated permit.
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>>;

    /// Checks at the border (B1...B4).
    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>>;

    /// Rechecks the permit together with the calling, installed transport.
    /// Old sources do not confirm such a binding and are obliged to refuse.
    fn revalidate_caller<'a>(
        &'a self,
        _caller: &'a CallerContext,
        _grant: &'a ValidatedGrant,
        _boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }
}
