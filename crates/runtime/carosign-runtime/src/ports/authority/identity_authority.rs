use super::super::BoxFuture;
use super::super::CallerContext;
use carosign_core::identity::IdentityError;
use carosign_core::identity::SubjectRef;
use carosign_core::identity::SubjectSelector;

/// A source of identities.
pub trait IdentityAuthority: Send + Sync {
    /// From an untested choice gets the subject.
    ///
    /// ⚠️ The choice is data, not authority: the port is obliged to check it with what the
    /// caller is really allowed.
    fn resolve(
        &self,
        caller: &CallerContext,
        selector: &SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, IdentityError>>;

    /// Allows a complete unverified verification request. The old port is not a replacement:
    /// it doesn't see the target and the link, so the default is a waiver.
    fn resolve_verification<'a>(
        &'a self,
        _caller: &'a CallerContext,
        _intent: &'a crate::authority::verification::VerificationIntent,
    ) -> BoxFuture<'a, Result<SubjectRef, IdentityError>> {
        Box::pin(async { Err(IdentityError::Unavailable) })
    }
}
