use super::*;

#[derive(Default)]
pub(super) struct TrackingAuthority {
    pub(super) authorized: std::sync::atomic::AtomicUsize,
    pub(super) revalidated: std::sync::atomic::AtomicUsize,
}

impl PolicyAuthority for TrackingAuthority {
    fn authorize(
        &self,
        _caller: &CallerContext,
        scope: GrantScope,
    ) -> BoxFuture<'_, Result<ValidatedGrant, AuthorityRefusal>> {
        self.authorized
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let grant = ValidatedGrant::issue_for_port(
            GrantId::from_bytes([42; 16]).expect("валид"),
            AuthorityId::from_bytes([2; 16]).expect("валид"),
            scope,
            BootId::from_bytes([9; 16]).expect("валид"),
            10_000,
        );
        Box::pin(async move { Ok(grant) })
    }

    fn revalidate(
        &self,
        grant: &ValidatedGrant,
        boundary: AuthorizationBoundary,
    ) -> BoxFuture<'_, Result<(), AuthorityRefusal>> {
        assert_eq!(grant.id().as_bytes(), &[42; 16]);
        assert_eq!(boundary, AuthorizationBoundary::BeforeDispatch);
        self.revalidated
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}
