use super::*;

pub(super) struct NeverBroker;

impl carosign_runtime::ports::ResourcePort for NeverBroker {
    fn acquire<'a>(
        &'a self,
        _: &'a carosign_core::resource::ResourceSet,
        _: LeaseId,
    ) -> BoxFuture<
        'a,
        Result<carosign_runtime::ports::ResourceLease, carosign_runtime::ports::ResourceDenial>,
    > {
        panic!("посредник не вызывается без профиля")
    }
    fn release(
        &self,
        _: carosign_runtime::ports::ResourceLease,
    ) -> BoxFuture<'_, carosign_runtime::ports::QuiesceAssessment> {
        panic!("выдачи не было")
    }
}
