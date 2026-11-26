use super::*;

pub(super) struct BeforeDispatch(pub(super) bool);

impl carosign_runtime::ports::PolicyAuthority for BeforeDispatch {
    fn subject_uid(
        &self,
        _: carosign_core::identity::SubjectRef,
    ) -> Result<u32, carosign_runtime::ports::AuthorityRefusal> {
        Ok(1000)
    }
    fn authorize(
        &self,
        _: &CallerContext,
        _: carosign_core::authorization::GrantScope,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<ValidatedGrant, carosign_runtime::ports::AuthorityRefusal>,
    > {
        panic!("проверяется существующее разрешение")
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        assert_eq!(
            boundary,
            carosign_core::authorization::AuthorizationBoundary::BeforeDispatch
        );
        let deny = self.0;
        Box::pin(async move {
            if deny {
                Err(carosign_runtime::ports::AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }
    fn revalidate_caller<'a>(
        &'a self,
        _: &'a CallerContext,
        _: &'a ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        assert_eq!(
            boundary,
            carosign_core::authorization::AuthorizationBoundary::BeforeDispatch
        );
        let deny = self.0;
        Box::pin(async move {
            if deny {
                Err(carosign_runtime::ports::AuthorityRefusal::Revoked)
            } else {
                Ok(())
            }
        })
    }
}
