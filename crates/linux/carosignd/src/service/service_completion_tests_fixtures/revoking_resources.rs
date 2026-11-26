use super::*;

pub(super) struct RevokingResources<'a> {
    pub(super) operations: &'a crate::operations::OperationRegistry,
}

impl ResourcePort for RevokingResources<'_> {
    fn acquire_for<'a>(
        &'a self,
        owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move {
            assert!(matches!(
                self.operations.revoke_verification(owner.operation),
                Err(crate::operations::ObservationFailure::Busy)
            ));
            Resources.acquire_for(owner, set).await
        })
    }
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move {
            let operation = OperationId::from_bytes([3; 16]).unwrap();
            assert!(matches!(
                self.operations.revoke_verification(operation),
                Err(crate::operations::ObservationFailure::Busy)
            ));
            Resources.acquire(set, lease).await
        })
    }
    fn release(&self, _: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        panic!("отозванная выдача требует подтверждённой уборки")
    }
}
