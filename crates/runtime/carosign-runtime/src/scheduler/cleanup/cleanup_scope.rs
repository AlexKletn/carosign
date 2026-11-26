use crate::ports::ResourceLease;
use crate::ports::ResourceOwner;
use carosign_core::ids::LeaseId;
use carosign_core::ids::ResourceId;
use carosign_core::resource::ResourceSet;

/// The entire area of one issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupScope {
    pub(super) lease: LeaseId,
    pub(super) owner: ResourceOwner,
    pub(super) resources: Vec<ResourceId>,
    pub(super) set: ResourceSet,
}

impl CleanupScope {
    /// Forms an area from withheld issuance; does not confirm its origin.
    pub fn of(lease: &ResourceLease) -> Self {
        Self {
            lease: lease.lease_id(),
            owner: lease.owner(),
            resources: lease.resources().to_vec(),
            set: lease.resource_set().clone(),
        }
    }
    /// Issue number.
    pub fn lease_id(&self) -> LeaseId {
        self.lease
    }
    /// Full owner.
    pub fn owner(&self) -> ResourceOwner {
        self.owner
    }
    /// Resource numbers.
    pub fn resources(&self) -> &[ResourceId] {
        &self.resources
    }
    /// Resource set with regimes.
    pub fn resource_set(&self) -> &ResourceSet {
        &self.set
    }
}
