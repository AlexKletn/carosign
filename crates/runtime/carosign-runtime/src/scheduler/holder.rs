use crate::ports::ResourceOwner;
use carosign_core::{device::DeviceEpoch, ids::LeaseId, resource::AccessMode};

/// Resource order: who is holding it now and what is the issue?
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    pub lease: LeaseId,
    /// Full owner of the issue.
    pub owner: ResourceOwner,
    /// The era of extradition.
    pub epoch: DeviceEpoch,
    /// In what mode.
    pub mode: AccessMode,
}
