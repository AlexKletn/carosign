use super::*;

pub(super) struct WaitingBroker(pub(super) std::sync::atomic::AtomicUsize);

impl carosign_runtime::ports::ResourcePort for WaitingBroker {
    fn acquire<'a>(
        &'a self,
        _: &'a carosign_core::resource::ResourceSet,
        _: LeaseId,
    ) -> BoxFuture<
        'a,
        Result<carosign_runtime::ports::ResourceLease, carosign_runtime::ports::ResourceDenial>,
    > {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
    fn release(
        &self,
        _: carosign_runtime::ports::ResourceLease,
    ) -> BoxFuture<'_, carosign_runtime::ports::QuiesceAssessment> {
        panic!("выдачи не было")
    }
}
