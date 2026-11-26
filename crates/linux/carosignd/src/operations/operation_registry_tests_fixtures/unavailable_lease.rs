pub(super) struct UnavailableLease;

impl crate::resources::MutationLeaseVerifier for UnavailableLease {
    fn validate<'a>(
        &'a self,
        _: &'a crate::resources::CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<(), carosign_runtime::ports::ResourceDenial>>
    {
        Box::pin(async { Err(carosign_runtime::ports::ResourceDenial::Unavailable) })
    }
}
