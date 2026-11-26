use super::{
    BoxFuture, CleanupPortFailure, CleanupScope, LeaseId, QuiesceAssessment, QuiescedLease,
    ResourceDenial, ResourceLease, ResourcePort, ResourceSet, TrustedCleanupBroker, fmt,
};

/// There's no broker.
///
/// ⚠️ Separate type, not “empty leasing”: the refusal must be different from the issuance.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NoBroker;

impl TrustedCleanupBroker for NoBroker {
    fn quiesce<'a>(
        &'a self,
        _scope: &'a CleanupScope,
    ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        Box::pin(async { Err(CleanupPortFailure::Unavailable) })
    }

    fn release<'a>(
        &'a self,
        _scope: &'a CleanupScope,
        _proof: QuiescedLease,
    ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
        Box::pin(async { Err(CleanupPortFailure::Unavailable) })
    }
}

impl ResourcePort for NoBroker {
    fn acquire<'a>(
        &'a self,
        _set: &'a ResourceSet,
        _lease_id: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        // ⚠️ Refusal with an honest word: the broker is not available.
        Box::pin(async { Err(ResourceDenial::Unavailable) })
    }

    fn release(&self, _lease: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        // ⚠️ There is nothing to release, but it is impossible to say “calm down”: without an
        // broker, the state of the device is unknown.
        Box::pin(async { QuiesceAssessment::Uncertain })
    }
}

impl fmt::Display for NoBroker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("посредник ресурсов недоступен")
    }
}
