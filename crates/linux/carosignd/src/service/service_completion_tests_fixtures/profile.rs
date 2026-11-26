use super::*;

pub(super) struct Profile;

impl crate::resources::ResourceSetSource for Profile {
    fn resource_set(&self, _: AuthenticatorRef) -> Result<ResourceSet, ResourceDenial> {
        Ok(ResourceSet::new(vec![ResourceClaim {
            id: ResourceId::from_bytes([8; 16]).unwrap(),
            mode: AccessMode::Exclusive,
        }])
        .unwrap())
    }
}
