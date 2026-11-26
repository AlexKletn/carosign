use super::*;

pub(super) struct Authority<'a> {
    pub(super) deny: bool,
    pub(super) inject: Option<(&'a OperationRegistry, OperationId)>,
}

impl carosign_runtime::ports::PolicyAuthority for Authority<'_> {
    fn revalidate_caller<'a>(
        &'a self,
        _: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<(), carosign_runtime::ports::AuthorityRefusal>,
    > {
        self.revalidate(grant, boundary)
    }
    fn authorize(
        &self,
        _: &CallerContext,
        _: carosign_core::authorization::GrantScope,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<ValidatedGrant, carosign_runtime::ports::AuthorityRefusal>,
    > {
        panic!("проверяется только повторная проверка")
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
            carosign_core::authorization::AuthorizationBoundary::BeforeDelivery
        );
        if let Some((registry, id)) = self.inject {
            let mut guard = registry.checkout(id).unwrap();
            guard
                .operation_mut()
                .actor
                .post(carosign_runtime::OperationMsg::GrantRevoked)
                .unwrap();
        }
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
