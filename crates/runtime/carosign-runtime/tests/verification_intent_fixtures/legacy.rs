use super::*;

pub(super) struct Legacy(pub(super) Authority);

impl PolicyAuthority for Legacy {
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        self.0.authorize(caller, scope)
    }

    fn authorize_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
        subject: SubjectRef,
    ) -> BoxFuture<'a, Result<ValidatedGrant, AuthorityRefusal>> {
        self.0.authorize_verification(caller, intent, subject)
    }

    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        self.0.revalidate(grant, boundary)
    }
}
