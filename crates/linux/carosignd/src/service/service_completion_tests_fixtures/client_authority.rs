use super::*;

pub(super) struct ClientAuthority {
    pub(super) caller: CallerContext,
    pub(super) subject: SubjectRef,
    pub(super) boot: BootId,
    pub(super) refusal: Option<AuthorityRefusal>,
}

impl IdentityAuthority for ClientAuthority {
    fn resolve(
        &self,
        _: &CallerContext,
        _: &carosign_core::identity::SubjectSelector,
    ) -> BoxFuture<'_, Result<SubjectRef, carosign_core::identity::IdentityError>> {
        Box::pin(async { Err(carosign_core::identity::IdentityError::Unavailable) })
    }
    fn resolve_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        _: &'a carosign_runtime::authority::verification::VerificationIntent,
    ) -> BoxFuture<'a, Result<SubjectRef, carosign_core::identity::IdentityError>> {
        Box::pin(async move {
            if caller == &self.caller {
                Ok(self.subject)
            } else {
                Err(carosign_core::identity::IdentityError::NotPermitted)
            }
        })
    }
}

impl PolicyAuthority for ClientAuthority {
    fn subject_uid(&self, subject: SubjectRef) -> Result<u32, AuthorityRefusal> {
        if subject == self.subject {
            Ok(1000)
        } else {
            Err(AuthorityRefusal::Denied)
        }
    }
    fn revalidate_caller<'a>(
        &'a self,
        caller: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        if caller != &self.caller {
            return Box::pin(async { Err(AuthorityRefusal::Denied) });
        }
        self.revalidate(grant, boundary)
    }
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        panic!("старый порт не должен вызываться")
    }
    fn authorize_verification<'a>(
        &'a self,
        caller: &'a CallerContext,
        intent: &'a carosign_runtime::authority::verification::VerificationIntent,
        subject: SubjectRef,
    ) -> BoxFuture<'a, Result<ValidatedGrant, AuthorityRefusal>> {
        assert!(caller == &self.caller && subject == self.subject);
        let scope = GrantScope::Verify {
            subject,
            authenticator: intent.authenticator,
            purpose: carosign_core::operation::context::VerificationPurpose::SessionUnlock,
        };
        Box::pin(async move {
            Ok(ValidatedGrant::issue_for_port(
                GrantId::from_bytes([14; 16]).unwrap(),
                AuthorityId::from_bytes([15; 16]).unwrap(),
                scope,
                self.boot,
                60_000_000_000,
            ))
        })
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        Box::pin(async move {
            if matches!(
                boundary,
                AuthorizationBoundary::BeforeCreation
                    | AuthorizationBoundary::BeforeDispatch
                    | AuthorizationBoundary::BeforeDelivery
            ) {
                self.refusal.map_or(Ok(()), Err)
            } else {
                Ok(())
            }
        })
    }
}
