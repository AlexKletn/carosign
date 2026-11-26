use super::*;

pub(super) struct ChangeDuringAuthorization<'a>(pub(super) &'a RedbStore);

impl PolicyAuthority for ChangeDuringAuthorization<'_> {
    fn authorize(
        &self,
        caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        let mut changed = record(7, EnrollmentStatus::Active);
        changed.enrollment.generation = EnrollmentGeneration::new(2).unwrap();
        put(self.0, &changed);
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
