use super::SCOPED_FENCE_MAGIC;

/// Context of sustainable fencing; does not in itself confirm cleaning.
pub struct FenceScope {
    pub lease: carosign_core::ids::LeaseId,
    pub operation: carosign_core::ids::OperationId,
    pub worker: carosign_core::ids::WorkerInstanceId,
    pub authenticator: carosign_core::device::AuthenticatorRef,
    pub resources: carosign_core::resource::ResourceSet,
}

impl FenceScope {
    pub(super) fn encoded_operation(value: &[u8]) -> Option<carosign_core::ids::OperationId> {
        carosign_core::ids::OperationId::try_from(value.get(20..36)?).ok()
    }

    pub(super) fn decode(value: &[u8]) -> Option<Self> {
        use carosign_core::{
            device::{AuthenticatorRef, DeviceEpoch},
            ids::*,
            resource::*,
        };

        super::scoped_fence_resources(value)?;
        let claims = value[77..]
            .chunks_exact(17)
            .map(|claim| {
                let mode = match claim[16] {
                    1 => AccessMode::Exclusive,
                    2 => AccessMode::SharedRead,
                    3 => AccessMode::Mediated,
                    _ => return None,
                };
                Some(ResourceClaim {
                    id: ResourceId::try_from(&claim[..16]).ok()?,
                    mode,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            lease: LeaseId::try_from(&value[4..20]).ok()?,
            operation: Self::encoded_operation(value)?,
            worker: WorkerInstanceId::try_from(&value[36..52]).ok()?,
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::try_from(&value[52..68]).ok()?,
                device_epoch: DeviceEpoch::new(u64::from_le_bytes(value[68..76].try_into().ok()?))
                    .ok()?,
            },
            resources: ResourceSet::new(claims).ok()?,
        })
    }

    pub(in super::super) fn encode(&self) -> Vec<u8> {
        use carosign_core::resource::AccessMode;
        let mut value = Vec::with_capacity(77 + self.resources.len() * 17);
        value.extend_from_slice(SCOPED_FENCE_MAGIC);
        value.extend_from_slice(self.lease.as_bytes());
        value.extend_from_slice(self.operation.as_bytes());
        value.extend_from_slice(self.worker.as_bytes());
        value.extend_from_slice(self.authenticator.id.as_bytes());
        value.extend_from_slice(&self.authenticator.device_epoch.value().to_le_bytes());
        value.push(self.resources.len() as u8);
        for claim in self.resources.claims() {
            value.extend_from_slice(claim.id.as_bytes());
            value.push(match claim.mode {
                AccessMode::Exclusive => 1,
                AccessMode::SharedRead => 2,
                AccessMode::Mediated => 3,
            });
        }
        value
    }
}
