use carosign_core::identity::SubjectRef;
use carosign_runtime::ports::{CallerContext, PolicyAuthority};

/// Substitutes the original caller when re-checking the already issued permit at the boundary
/// of dispatch.
pub(super) struct CallerBoundAuthority<'a, A> {
    pub(super) authority: &'a A,
    pub(super) caller: &'a CallerContext,
}

impl<A: PolicyAuthority> PolicyAuthority for CallerBoundAuthority<'_, A> {
    fn subject_uid(
        &self,
        subject: SubjectRef,
    ) -> Result<u32, carosign_runtime::ports::AuthorityRefusal> {
        self.authority.subject_uid(subject)
    }
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: carosign_core::authorization::GrantScope,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<
            carosign_core::authorization::ValidatedGrant,
            carosign_runtime::ports::AuthorityRefusal,
        >,
    > {
        self.authority.authorize(caller, scope)
    }
    fn revalidate(
        &self,
        grant: &carosign_core::authorization::ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        let grant = grant.clone();
        Box::pin(async move {
            self.authority
                .revalidate_caller(self.caller, &grant, boundary)
                .await
        })
    }
    fn revalidate_caller<'a>(
        &'a self,
        caller: &'a CallerContext,
        grant: &'a carosign_core::authorization::ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        self.authority.revalidate_caller(caller, grant, boundary)
    }
}
