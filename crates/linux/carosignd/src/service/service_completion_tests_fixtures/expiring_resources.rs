use super::*;

pub(super) struct ExpiringResources<'a> {
    pub(super) clock: &'a carosign_adapters::clock::StepClock,
}

impl ResourcePort for ExpiringResources<'_> {
    fn acquire_for<'a>(
        &'a self,
        owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move {
            self.clock.advance(61_000_000_000);
            Resources.acquire_for(owner, set).await
        })
    }
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move {
            self.clock.advance(61_000_000_000);
            Resources.acquire(set, lease).await
        })
    }
    fn release(&self, _: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        panic!("выдача после срока требует подтверждённой уборки")
    }
}
