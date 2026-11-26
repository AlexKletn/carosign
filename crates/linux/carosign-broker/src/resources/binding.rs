use super::{BindingContext, Claim, ClaimError, FdRole, ResourceKind};

use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{AuthenticatorId, LeaseId, ResourceId, WorkerInstanceId};

/// Who's issued.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Binding {
    pub lease: LeaseId,
    pub worker: WorkerInstanceId,
    pub authenticator: AuthenticatorId,
    pub epoch: DeviceEpoch,
    pub resource: ResourceId,
    pub role: FdRole,
}

/// Is this combination of role and carrier acceptable?
///
/// ⚠️ The roles are not interchangeable: `Device` It can't be a file. `Config` - cannot be
/// recorded, `MediatedService` It cannot be a bare description of the device (it would be
/// bypassing the broker).
pub fn role_fits(role: FdRole, kind: ResourceKind) -> bool {
    matches!(
        (role, kind),
        (FdRole::Device, ResourceKind::CharacterDevice)
            | (FdRole::Event, ResourceKind::EventFd)
            | (FdRole::Config, ResourceKind::SealedMemfd)
            | (FdRole::MediatedService, ResourceKind::SocketPair)
    )
}

/// Checking the delivery of the set.
///
/// ⚠️ It is all or nothing: a set that is given out piecemeal cannot be released by one
/// action.
pub fn validate(
    claims: &[Claim],
    context: BindingContext,
    current_epoch: DeviceEpoch,
    granted: &[ResourceId],
) -> Result<Vec<Binding>, ClaimError> {
    if context.authenticator.device_epoch != current_epoch {
        return Err(ClaimError::StaleEpoch);
    }
    let mut seen: Vec<ResourceId> = Vec::new();
    let mut bindings = Vec::with_capacity(claims.len());
    for claim in claims {
        if !role_fits(claim.role, claim.kind) {
            return Err(ClaimError::RoleMismatch {
                role: claim.role,
                kind: claim.kind,
            });
        }
        if seen.contains(&claim.resource) {
            return Err(ClaimError::Duplicate {
                resource: claim.resource,
            });
        }
        if !granted.contains(&claim.resource) {
            return Err(ClaimError::NotInLease {
                resource: claim.resource,
            });
        }
        seen.push(claim.resource);
        bindings.push(Binding {
            lease: context.lease,
            worker: context.worker,
            authenticator: context.authenticator.id,
            epoch: context.authenticator.device_epoch,
            resource: claim.resource,
            role: claim.role,
        });
    }
    Ok(bindings)
}
