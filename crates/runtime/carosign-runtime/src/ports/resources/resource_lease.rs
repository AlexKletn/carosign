use super::ResourceOwner;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::LeaseId;
use carosign_core::ids::ResourceId;
use carosign_core::resource::ResourceSet;

#[cfg(all(feature = "resource-fixtures", not(debug_assertions)))]
compile_error!("resource-fixtures предназначен только для отладочных проверок");

/// A set of resources issued.
///
/// ⚠️ The fields are closed; the issue is moved with the owner and is not cloned.
/// The production designer will appear together with a trusted broker.
#[derive(PartialEq, Eq)]
pub struct ResourceLease {
    lease: LeaseId,
    owner: ResourceOwner,
    resources: Vec<ResourceId>,
    set: ResourceSet,
}

impl ResourceLease {
    /// Forms a portable description of the issue already created by the port owner.
    /// The description itself does not confirm the right: the owner is obliged to check it
    /// according to his living record at each use and release.
    pub fn issued(lease: LeaseId, owner: ResourceOwner, set: ResourceSet) -> Self {
        let resources = set.claims().iter().map(|claim| claim.id).collect();
        Self {
            lease,
            owner,
            resources,
            set,
        }
    }

    /// Only for a front broker in debugging tests.
    #[cfg(all(feature = "resource-fixtures", debug_assertions))]
    pub fn issued_for_fixture(lease: LeaseId, owner: ResourceOwner, set: ResourceSet) -> Self {
        Self::issued(lease, owner, set)
    }

    /// Issue number.
    pub fn lease_id(&self) -> LeaseId {
        self.lease
    }

    /// The era at the time of issuance.
    pub fn epoch(&self) -> DeviceEpoch {
        self.owner.authenticator.device_epoch
    }

    /// The operation, worker and device to which the issuance relates.
    pub fn owner(&self) -> ResourceOwner {
        self.owner
    }

    /// That's issued.
    pub fn resources(&self) -> &[ResourceId] {
        &self.resources
    }

    /// The exact canonical set, including the access mode of each resource.
    pub fn resource_set(&self) -> &ResourceSet {
        &self.set
    }
}

impl core::fmt::Debug for ResourceLease {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResourceLease")
            .field("resources", &self.resources.len())
            .finish_non_exhaustive()
    }
}
