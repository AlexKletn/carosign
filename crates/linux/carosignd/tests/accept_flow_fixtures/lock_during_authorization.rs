use super::*;

pub(super) struct LockDuringAuthorization<'a>(pub(super) &'a RedbStore);

impl PolicyAuthority for LockDuringAuthorization<'_> {
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        let mut tx = self.0.begin_write().unwrap();
        tx.put(
            Table::MutationScopeLock,
            &scope_of(subject(), authenticator()),
            &[9; 16],
        )
        .unwrap();
        tx.commit_durable().unwrap();
        Allow.authorize(caller, scope)
    }
    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        boundary: carosign_core::authorization::AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        Allow.revalidate(grant, boundary)
    }
}
