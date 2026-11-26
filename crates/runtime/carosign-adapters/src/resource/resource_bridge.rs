use super::BrokerClient;

use carosign_core::ids::LeaseId;
use carosign_core::resource::ResourceSet;
use carosign_runtime::ports::{QuiesceAssessment, ResourceDenial, ResourceLease, ResourcePort};

/// Resource port adapter.
pub struct ResourceBridge<B: BrokerClient> {
    _broker: B,
}

impl<B: BrokerClient> core::fmt::Debug for ResourceBridge<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ResourceBridge(..)")
    }
}

impl<B: BrokerClient> ResourceBridge<B> {
    /// Adapter on top of the middleman.
    pub fn new(broker: B) -> Self {
        Self { _broker: broker }
    }
}

impl<B: BrokerClient> ResourcePort for ResourceBridge<B> {
    fn acquire<'a>(
        &'a self,
        _set: &'a ResourceSet,
        _lease_id: LeaseId,
    ) -> carosign_runtime::BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        // The old contract doesn't hand over the owner, and it doesn't securely remove the
        // remittance, and you can't ask for a real resource through it.
        Box::pin(async { Err(ResourceDenial::Unavailable) })
    }

    fn release(
        &self,
        _lease: ResourceLease,
    ) -> carosign_runtime::BoxFuture<'_, QuiesceAssessment> {
        // An issue number without an owner and an era cannot allow release.
        Box::pin(async { QuiesceAssessment::Uncertain })
    }
}
