use super::*;

pub(super) struct Resources;

impl ResourcePort for Resources {
    fn acquire_for<'a>(
        &'a self,
        owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        let set = set.clone();
        Box::pin(async move {
            Ok(ResourceLease::issued_for_fixture(
                LeaseId::from_bytes([10; 16]).unwrap(),
                owner,
                set,
            ))
        })
    }
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        let owner = ResourceOwner {
            operation: OperationId::from_bytes([3; 16]).unwrap(),
            worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::from_bytes([7; 16]).unwrap(),
                device_epoch: DeviceEpoch::FIRST,
            },
        };
        let set = set.clone();
        Box::pin(async move { Ok(ResourceLease::issued_for_fixture(lease, owner, set)) })
    }
    fn release(&self, _: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        panic!("старый порт уборки не используется")
    }
}
