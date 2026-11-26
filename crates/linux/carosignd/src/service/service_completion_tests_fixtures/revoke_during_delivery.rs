use super::*;

pub(super) struct RevokeDuringDelivery<'a> {
    pub(super) operations: &'a crate::operations::OperationRegistry,
    pub(super) operation: OperationId,
}

impl PolicyAuthority for RevokeDuringDelivery<'_> {
    fn revalidate_caller<'a>(
        &'a self,
        _: &'a CallerContext,
        grant: &'a ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'a, Result<(), AuthorityRefusal>> {
        self.revalidate(grant, boundary)
    }
    fn authorize(
        &self,
        _: &CallerContext,
        _: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        panic!("вызывается только повторная проверка")
    }
    fn revalidate(
        &self,
        _: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        assert_eq!(boundary, AuthorizationBoundary::BeforeDelivery);
        let _ = self.operations.revoke_verification(self.operation);
        Box::pin(async { Ok(()) })
    }
}
