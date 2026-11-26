use super::AccessMode;
use super::DelegationError;
use super::HandleKind;
use super::Role;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::ResourceId;

/// Reference: what is issued, to whom and at what era.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Delegation {
    pub resource: ResourceId,
    pub role: Role,
    pub epoch: DeviceEpoch,
}

impl Delegation {
    /// Whether the descriptor is fit for that reference.
    pub fn accepts(self, kind: HandleKind, mode: AccessMode) -> bool {
        kind == self.role.kind() && mode.fits(kind)
    }
}

/// Checks to see if I can give it back.
///
/// ⚠️ Refusal to transfer: after transfer can not be returned.
pub fn may_delegate(
    delegation: Delegation,
    current_epoch: DeviceEpoch,
    kind: HandleKind,
    mode: AccessMode,
) -> Result<(), DelegationError> {
    if delegation.epoch != current_epoch {
        return Err(DelegationError::StaleEpoch);
    }
    if kind != delegation.role.kind() {
        return Err(DelegationError::WrongKind);
    }
    if !mode.fits(kind) {
        return Err(DelegationError::TooBroad);
    }
    Ok(())
}
