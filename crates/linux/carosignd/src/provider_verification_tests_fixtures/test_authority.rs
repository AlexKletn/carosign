pub(super) struct TestAuthority<'a> {
    pub(super) clock: &'a carosign_adapters::clock::StepClock,
    pub(super) deny: bool,
    pub(super) advance: u64,
}

impl carosign_runtime::ports::PolicyAuthority for TestAuthority<'_> {
    fn subject_uid(
        &self,
        _: carosign_core::identity::SubjectRef,
    ) -> Result<u32, carosign_runtime::ports::AuthorityRefusal> {
        Ok(1000)
    }
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
            carosign_core::authorization::AuthorizationBoundary::BeforeDispatch
        );
        self.clock.advance(self.advance);
        let deny = self.deny;
        Box::pin(async move {
            if deny {
                Err(carosign_runtime::ports::AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }
}
