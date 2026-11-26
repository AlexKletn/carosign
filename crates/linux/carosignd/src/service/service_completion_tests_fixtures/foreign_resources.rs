use super::*;

pub(super) struct ForeignResources;

impl ResourcePort for ForeignResources {
    fn acquire_for<'a>(
        &'a self,
        _owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        self.acquire(set, LeaseId::from_bytes([10; 16]).unwrap())
    }
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        let owner = ResourceOwner {
            operation: OperationId::from_bytes([99; 16]).unwrap(),
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
        panic!("чужая выдача не освобождается автоматически")
    }
}
