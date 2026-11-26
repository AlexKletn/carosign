use super::*;

pub(super) struct Authority {
    pub(super) expected: VerificationIntent,
    pub(super) wrong_scope: bool,
    pub(super) revoked: bool,
}

impl IdentityAuthority for Authority {
    fn resolve(
        &self,
        _: &CallerContext,
        _: &SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, IdentityError>> {
        Box::pin(async { Err(IdentityError::Unavailable) })
    }
    fn resolve_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
    ) -> BoxFuture<'a, Result<SubjectRef, IdentityError>> {
        assert!(caller == &crate::caller() && intent == &self.expected);
        Box::pin(async { Ok(subject()) })
    }
}

impl PolicyAuthority for Authority {
    fn revalidate_caller<'a>(
        &'a self,
        caller: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        if caller != &crate::caller() {
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        }
        self.revalidate(grant, boundary)
    }
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        Box::pin(async { Err(AuthorityRefusal::Unavailable) })
    }
    fn authorize_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a VerificationIntent,
        resolved: SubjectRef,
    ) -> BoxFuture<'a, Result<ValidatedGrant, AuthorityRefusal>> {
        assert!(caller == &crate::caller() && intent == &self.expected && resolved == subject());
        let authenticator = if self.wrong_scope {
            AuthenticatorId::from_bytes([9; 16]).unwrap()
        } else {
            intent.authenticator
        };
        Box::pin(async move {
            Ok(ValidatedGrant::issue_for_port(
                GrantId::from_bytes([6; 16]).unwrap(),
                AuthorityId::from_bytes([1; 16]).unwrap(),
                GrantScope::Verify {
                    subject: resolved,
                    authenticator,
                    purpose: VerificationPurpose::SessionUnlock,
                },
                BootId::from_bytes([7; 16]).unwrap(),
                100,
            ))
        })
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        assert!(boundary == AuthorizationBoundary::BeforeCreation);
        let revoked = self.revoked;
        Box::pin(async move {
            if revoked {
                Err(AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }
}
