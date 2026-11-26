pub(super) struct ExpiresLease<'a>(pub(super) &'a carosign_adapters::clock::StepClock);

impl crate::resources::MutationLeaseVerifier for ExpiresLease<'_> {
    fn validate<'a>(
        &'a self,
        _: &'a crate::resources::CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<(), carosign_runtime::ports::ResourceDenial>>
    {
        self.0.advance(30_000_000_000);
        Box::pin(async { Ok(()) })
    }
}
