use super::*;
use carosign_runtime::ports::{
    QuiesceAssessment, ResourceDenial, ResourceLease, ResourceOwner, ResourcePort,
};

pub(super) struct ForeignOwner {
    pub(super) mismatch: u8,
}

impl ResourcePort for ForeignOwner {
    fn acquire<'a>(
        &'a self,
        set: &'a carosign_core::resource::ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        let mut operation = [0; 16];
        operation[15] = if self.mismatch == 0 { 9 } else { 1 };
        let owner = ResourceOwner {
            operation: OperationId::from_bytes(operation).unwrap(),
            worker: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
            authenticator: carosign_core::device::AuthenticatorRef {
                id: if self.mismatch == 1 {
                    AuthenticatorId::from_bytes([9; 16]).unwrap()
                } else {
                    authenticator()
                },
                device_epoch: DeviceEpoch::FIRST,
            },
        };
        let resources = if self.mismatch == 2 {
            carosign_core::resource::ResourceSet::new(
                set.claims()
                    .iter()
                    .map(|claim| carosign_core::resource::ResourceClaim {
                        id: claim.id,
                        mode: carosign_core::resource::AccessMode::SharedRead,
                    })
                    .collect(),
            )
            .unwrap()
        } else {
            set.clone()
        };
        Box::pin(async move { Ok(ResourceLease::issued_for_fixture(lease, owner, resources)) })
    }
    fn release(&self, _: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        panic!("чужую выдачу нельзя освобождать по предположению")
    }
}
