use crate::ports::ResourceOwner;
use carosign_core::ids::LeaseId;
use carosign_core::resource::ResourceSet;

/// Disposable holder of the full area of cleaning, loss does not remove the fence.
pub struct Quiescing {
    pub(in crate::scheduler) lease: LeaseId,
    pub(in crate::scheduler) owner: ResourceOwner,
    pub(in crate::scheduler) set: ResourceSet,
    pub(in crate::scheduler) key: std::sync::Arc<()>,
}

impl Quiescing {
    /// Withholded extradition number.
    pub fn lease_id(&self) -> LeaseId {
        self.lease
    }
    /// Full owner of the withheld extradition.
    pub fn owner(&self) -> ResourceOwner {
        self.owner
    }
    /// Canonical set and regimes.
    pub fn resource_set(&self) -> &ResourceSet {
        &self.set
    }
}
