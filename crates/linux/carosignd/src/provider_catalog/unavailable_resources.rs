pub(crate) struct UnavailableResources;

impl crate::resources::ResourceSetSource for UnavailableResources {
    fn resource_set(
        &self,
        _: carosign_core::device::AuthenticatorRef,
    ) -> Result<carosign_core::resource::ResourceSet, carosign_runtime::ports::ResourceDenial>
    {
        Err(carosign_runtime::ports::ResourceDenial::Unavailable)
    }
}
