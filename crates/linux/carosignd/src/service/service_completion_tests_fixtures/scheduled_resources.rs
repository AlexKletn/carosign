use super::*;

/// Testing and cleaning use a single scheduler.
pub(super) struct ScheduledResources {
    pub(super) scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
    pub(super) clock: std::sync::Arc<carosign_adapters::clock::StepClock>,
}

impl ResourcePort for ScheduledResources {
    fn acquire_for<'a>(
        &'a self,
        owner: ResourceOwner,
        set: &'a ResourceSet,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move {
            use carosign_runtime::scheduler::{GrantResult, Priority, Request};
            let lease = LeaseId::from_bytes([10; 16]).unwrap();
            let request = Request {
                owner,
                priority: Priority::SystemAuth,
                set: set.clone(),
                since: self.clock.now(),
                mutating: false,
            };
            match self.scheduler.lock().unwrap().acquire(request, lease) {
                GrantResult::Granted { .. } => {
                    Ok(ResourceLease::issued_for_fixture(lease, owner, set.clone()))
                }
                GrantResult::Queued => Err(ResourceDenial::Busy),
                GrantResult::Denied => Err(ResourceDenial::Denied),
            }
        })
    }
    fn acquire<'a>(
        &'a self,
        set: &'a ResourceSet,
        lease: LeaseId,
    ) -> BoxFuture<'a, Result<ResourceLease, ResourceDenial>> {
        Box::pin(async move {
            use carosign_runtime::scheduler::{GrantResult, Priority, Request};
            let owner = ResourceOwner {
                operation: OperationId::from_bytes([3; 16]).unwrap(),
                worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
                authenticator: AuthenticatorRef {
                    id: AuthenticatorId::from_bytes([7; 16]).unwrap(),
                    device_epoch: DeviceEpoch::FIRST,
                },
            };
            let request = Request {
                owner,
                priority: Priority::SystemAuth,
                set: set.clone(),
                since: self.clock.now(),
                mutating: false,
            };
            match self.scheduler.lock().unwrap().acquire(request, lease) {
                GrantResult::Granted { .. } => {
                    Ok(ResourceLease::issued_for_fixture(lease, owner, set.clone()))
                }
                GrantResult::Queued => Err(ResourceDenial::Busy),
                GrantResult::Denied => Err(ResourceDenial::Denied),
            }
        })
    }
    fn release(&self, _: ResourceLease) -> BoxFuture<'_, QuiesceAssessment> {
        panic!("используется ограждённая уборка")
    }
}
