use super::*;

pub(super) struct ForeignWorkerResources;

impl ResourcePort for ForeignWorkerResources {
    fn acquire_for<'a>(
        &'a self,
        owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        let foreign = ResourceOwner {
            worker: WorkerInstanceId::from_bytes([99; 16]).unwrap(),
            ..owner
        };
        let set = set.clone();
        Box::pin(async move {
            Ok(ResourceLease::issued_for_fixture(
                LeaseId::from_bytes([10; 16]).unwrap(),
                foreign,
                set,
            ))
        })
    }
    fn acquire<'a>(
        &'a self,
        _: &'a ResourceSet,
        _: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        panic!("для проверки используется запрос с полным владельцем")
    }
    fn release(&self, _: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        panic!("чужая выдача требует подтверждённой уборки")
    }
}
