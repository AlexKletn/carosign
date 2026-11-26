use super::*;

pub(super) struct DelayedGrant<'a>(pub(super) &'a AdvancingClock);

impl carosign_runtime::ports::ResourcePort for DelayedGrant<'_> {
    fn acquire<'a>(
        &'a self,
        set: &'a carosign_core::resource::ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<
        'a,
        Result<carosign_runtime::ports::ResourceLease, carosign_runtime::ports::ResourceDenial>,
    > {
        Box::pin(async move {
            self.0
                .0
                .store(30_000_001_000, std::sync::atomic::Ordering::SeqCst);
            GrantAll {
                epoch: DeviceEpoch::FIRST,
            }
            .acquire(set, lease)
            .await
        })
    }

    fn release(
        &self,
        _lease: carosign_runtime::ports::ResourceLease,
    ) -> BoxFuture<'_, carosign_runtime::ports::QuiesceAssessment> {
        Box::pin(async { carosign_runtime::ports::QuiesceAssessment::Uncertain })
    }
}
