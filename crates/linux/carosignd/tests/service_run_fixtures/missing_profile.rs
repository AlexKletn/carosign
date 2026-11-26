use super::*;

pub(super) struct MissingProfile;

impl carosignd::resources::ResourceSetSource for MissingProfile {
    fn resource_set(
        &self,
        device: carosign_core::device::AuthenticatorRef,
    ) -> Result<carosign_core::resource::ResourceSet, carosign_runtime::ports::ResourceDenial>
    {
        assert_eq!(device.id, authenticator());
        assert_eq!(device.device_epoch, DeviceEpoch::FIRST);
        Err(carosign_runtime::ports::ResourceDenial::Unavailable)
    }
}
