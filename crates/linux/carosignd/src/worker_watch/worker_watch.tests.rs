use super::*;
use carosign_core::{
    device::AuthenticatorRef,
    ids::{AuthenticatorId, BootId, LeaseId, OperationId, ResourceId},
    operation::context::BootInstant,
    resource::{AccessMode, ResourceClaim, ResourceSet},
};
use carosign_runtime::{
    ports::ResourceOwner,
    scheduler::{FenceStage, GrantResult, Priority, Request},
};

#[test]
fn exited_child_fences_exact_worker_without_releasing_resources() {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let pidfd = PidFd::open(child.id()).unwrap();
    let worker = WorkerInstanceId::from_bytes([1; 16]).unwrap();
    let operation = OperationId::from_bytes([2; 16]).unwrap();
    let lease = LeaseId::from_bytes([3; 16]).unwrap();
    let resource = ResourceId::from_bytes([4; 16]).unwrap();
    let owner = ResourceOwner {
        operation,
        worker,
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([5; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let set = ResourceSet::new(vec![ResourceClaim {
        id: resource,
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    let request = Request {
        owner,
        priority: Priority::SystemAuth,
        set,
        since: BootInstant::new(BootId::from_bytes([6; 16]).unwrap(), 0),
        mutating: false,
    };
    let scheduler = Mutex::new(scheduler);
    let mut watch = WorkerWatch::new(pidfd, worker, DeviceEpoch::FIRST);
    assert!(matches!(
        watch
            .acquire_if_alive(&scheduler, request.clone(), lease)
            .unwrap(),
        GrantResult::Granted { .. }
    ));
    // The second operation of the same worker is waiting for the resource to be occupied,
    // and death should protect the application that has not yet been issued, not just the
    // holder.
    let mut queued = request.clone();
    queued.owner.operation = OperationId::from_bytes([7; 16]).unwrap();
    let queued_lease = LeaseId::from_bytes([8; 16]).unwrap();
    assert_eq!(
        watch
            .acquire_if_alive(&scheduler, queued.clone(), queued_lease)
            .unwrap(),
        GrantResult::Queued
    );
    assert_eq!(watch.poll(&scheduler).unwrap(), WatchStage::Watching);
    child.kill().unwrap();
    child.wait().unwrap();
    let mut after_exit = request.clone();
    after_exit.owner.operation = OperationId::from_bytes([9; 16]).unwrap();
    assert_eq!(
        watch
            .acquire_if_alive(
                &scheduler,
                after_exit,
                LeaseId::from_bytes([10; 16]).unwrap()
            )
            .unwrap(),
        GrantResult::Denied
    );
    assert_eq!(
        watch.poll(&scheduler).unwrap(),
        WatchStage::Fenced { leases: 1 }
    );
    assert_eq!(
        watch.poll(&scheduler).unwrap(),
        WatchStage::Fenced { leases: 1 }
    );
    let mut scheduler = scheduler.lock().unwrap();
    assert_eq!(scheduler.fence_stage(lease), Some(FenceStage::Uncertain));
    assert!(scheduler.release(lease).is_empty());
    assert_eq!(scheduler.holder_of(resource), Some(operation));
    let expired = scheduler.take_expired_requests();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].request, queued);
    assert!(scheduler.take_expired_requests().is_empty());
    let mut after_exit = request;
    after_exit.owner.operation = OperationId::from_bytes([9; 16]).unwrap();
    assert_eq!(
        scheduler.acquire(after_exit, LeaseId::from_bytes([10; 16]).unwrap()),
        GrantResult::Denied
    );
}
