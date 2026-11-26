use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::ids::{AuthenticatorId, LeaseId, OperationId, WorkerInstanceId};
use carosign_core::resource::ResourceSet;
use carosign_runtime::ports::{
    BoxFuture, QuiesceAssessment, ResourceDenial, ResourceLease, ResourceOwner, ResourcePort,
};

/// A positive response does not confirm the security of the device.
pub struct GrantAll {
    pub epoch: DeviceEpoch,
}

impl ResourcePort for GrantAll {
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease_id: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        assert_eq!(set.claims().len(), 1);
        assert_eq!(set.claims()[0].id.as_bytes(), &[42; 16]);
        let resources = set.clone();
        let epoch = self.epoch;
        let mut operation = [0; 16];
        operation[15] = 1;
        let owner = ResourceOwner {
            operation: OperationId::from_bytes(operation).unwrap(),
            worker: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::from_bytes([3; 16]).unwrap(),
                device_epoch: epoch,
            },
        };
        Box::pin(async move {
            Ok(ResourceLease::issued_for_fixture(
                lease_id, owner, resources,
            ))
        })
    }

    fn release(&self, _lease: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        Box::pin(async { QuiesceAssessment::Quiesced })
    }
}
