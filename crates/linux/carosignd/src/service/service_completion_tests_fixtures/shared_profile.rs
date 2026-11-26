use super::*;

pub(super) struct SharedProfile;

impl crate::resources::ResourceSetSource for SharedProfile {
    fn resource_set(&self, _: AuthenticatorRef) -> Result<ResourceSet, ResourceDenial> {
        Ok(ResourceSet::new(vec![ResourceClaim {
            id: ResourceId::from_bytes([8; 16]).unwrap(),
            mode: AccessMode::SharedRead,
        }])
        .unwrap())
    }
}
