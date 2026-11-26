use super::*;

pub(super) struct SharedCleanup<'a> {
    pub(super) scheduler: &'a std::sync::Mutex<carosign_runtime::scheduler::Scheduler>,
    pub(super) boot: BootId,
}

impl crate::resources::TrustedCleanupBroker for SharedCleanup<'_> {
    fn quiesce<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
    ) -> BoxFuture<'a, Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>>
    {
        Box::pin(async move {
            use carosign_runtime::scheduler::{GrantResult, Priority, Request};
            let other = ResourceOwner {
                operation: OperationId::from_bytes([95; 16]).unwrap(),
                ..scope.owner()
            };
            let result = self.scheduler.lock().unwrap().acquire(
                Request {
                    owner: other,
                    set: scope.resource_set().clone(),
                    priority: Priority::SystemAuth,
                    since: carosign_core::operation::context::BootInstant::new(self.boot, 0),
                    mutating: false,
                },
                LeaseId::from_bytes([96; 16]).unwrap(),
            );
            assert_eq!(result, GrantResult::Queued);
            Ok(scope.clone())
        })
    }
    fn release<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
        proof: crate::resources::QuiescedLease,
    ) -> BoxFuture<'a, Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>>
    {
        assert_eq!(proof.scope(), scope);
        Box::pin(async move { Ok(scope.clone()) })
    }
}
