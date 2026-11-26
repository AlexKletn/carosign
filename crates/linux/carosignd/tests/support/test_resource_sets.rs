use carosign_core::device::AuthenticatorRef;
use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
use carosign_runtime::ports::ResourceDenial;

/// Test profile with resource identifier separate from the device.
pub struct TestResourceSets;

impl carosignd::resources::ResourceSetSource for TestResourceSets {
    fn resource_set(
        &self,
        _authenticator: AuthenticatorRef,
    ) -> Result<ResourceSet, ResourceDenial> {
        Ok(ResourceSet::new(vec![ResourceClaim {
            id: carosign_core::ids::ResourceId::from_bytes([42; 16]).unwrap(),
            mode: AccessMode::Exclusive,
        }])
        .unwrap())
    }
}
