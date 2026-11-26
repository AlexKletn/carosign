use super::super::BoxFuture;
use super::QuiesceAssessment;
use super::ResourceDenial;
use super::ResourceLease;
use super::ResourceOwner;
use carosign_core::ids::LeaseId;
use carosign_core::resource::ResourceSet;

/// Resources.
pub trait ResourcePort: Send + Sync {
    /// Requests an exact set for the full owner. ID appoints an broker.
    /// The form of the owner does not in itself confirm the authority or process; an
    /// unavailable implementation is obliged to refuse before the issuance is created.
    fn acquire_for<'a>(
        &'a self,
        _owner: ResourceOwner,
        _set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async { Err(ResourceDenial::Unavailable) })
    }

    /// Takes the whole set and atomically.
    ///
    /// ⚠️ There is no partial issuance: either the whole set or refusal.
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease_id: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>>;

    /// It's liberating.
    fn release(&self, lease: ResourceLease) -> BoxFuture<'_, QuiesceAssessment>;
}
