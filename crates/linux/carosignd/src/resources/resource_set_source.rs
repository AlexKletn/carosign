use super::{ResourceDenial, ResourceSet};

/// Source of resources of the selected authenticator profile.
///
/// The implementation is chosen by the trusted composition of the platform. The returned set
/// is not per se permission and does not confirm the issuance by the broker.
pub trait ResourceSetSource: Send + Sync {
    /// Returns a complete set for a specific anchor and era.
    fn resource_set(
        &self,
        authenticator: carosign_core::device::AuthenticatorRef,
    ) -> Result<ResourceSet, ResourceDenial>;
}
