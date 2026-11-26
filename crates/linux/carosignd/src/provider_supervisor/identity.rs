/// Identity of device and set of resources from system profile.
#[derive(Clone)]
pub struct ProviderIdentity {
    pub authenticator: carosign_core::ids::AuthenticatorId,
    pub package: carosign_core::ids::PackageId,
    pub profile: carosign_core::ids::ProfileId,
    pub resources: carosign_core::resource::ResourceSet,
    pub modality: carosign_protocol::carosign::common::v1::Modality,
}

impl ProviderIdentity {
    /// Description of the test path declared by the device.
    pub fn descriptor(
        &self,
        epoch: carosign_core::device::DeviceEpoch,
    ) -> carosign_protocol::carosign::common::v1::AuthenticatorDescriptor {
        use carosign_protocol::carosign::common::v1::{
            AuthenticatorDescriptor, CapabilityDescriptor,
        };
        AuthenticatorDescriptor {
            authenticator_id: self.authenticator.as_bytes().to_vec(),
            device_epoch: epoch.value(),
            capabilities: Some(CapabilityDescriptor {
                modality: self.modality as i32,
                ..Default::default()
            }),
            available: false,
            unavailability_reason: 0,
        }
    }
}

impl crate::resources::ResourceSetSource for ProviderIdentity {
    fn resource_set(
        &self,
        authenticator: carosign_core::device::AuthenticatorRef,
    ) -> Result<carosign_core::resource::ResourceSet, carosign_runtime::ports::ResourceDenial> {
        if authenticator.id != self.authenticator {
            return Err(carosign_runtime::ports::ResourceDenial::Denied);
        }
        Ok(self.resources.clone())
    }
}
