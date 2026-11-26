use carosign_core::ids::WorkerInstanceId;

pub(super) struct LiveLease;

impl crate::resources::MutationLeaseVerifier for LiveLease {
    fn validate<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<(), carosign_runtime::ports::ResourceDenial>>
    {
        assert_eq!(
            scope.owner().worker,
            WorkerInstanceId::from_bytes([43; 16]).unwrap()
        );
        assert_eq!(
            scope.resource_set().claims()[0].mode,
            carosign_core::resource::AccessMode::Exclusive
        );
        Box::pin(async { Ok(()) })
    }
}
