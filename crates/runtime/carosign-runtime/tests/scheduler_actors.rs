//! Scheduler, Owner, Observers, and the Stream of Events
//!
//! These tests cover atomic resource acquisition, priority by class before arrival time,
//! starvation avoidance without preemption, bounded queues and observers, and explicit
//! failure.

use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{BootId, LeaseId, OperationId, ResourceId};
use carosign_core::operation::context::BootInstant;
use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
use carosign_runtime::CallerContext;
use carosign_runtime::operation::events::{EventKind, EventStream, HISTORY_MAX};
use carosign_runtime::operation::observer::{OBSERVERS_MAX, Observer, Observers};
use carosign_runtime::operation::owner::{OwnerChannel, OwnerHandle};
use carosign_runtime::ports::TransactionId;
use carosign_runtime::scheduler::{
    GrantResult, Priority, QUEUE_MAX, Request, Scheduler, is_starving,
};

fn resource(byte: u8) -> ResourceId {
    ResourceId::from_bytes([byte; 16]).expect("валид")
}

fn set(items: &[(u8, AccessMode)]) -> ResourceSet {
    ResourceSet::new(
        items
            .iter()
            .map(|(byte, mode)| ResourceClaim {
                id: resource(*byte),
                mode: *mode,
            })
            .collect(),
    )
    .expect("набор верен")
}

fn lease(byte: u8) -> LeaseId {
    LeaseId::from_bytes([byte; 16]).expect("валид")
}

fn operation(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).expect("валид")
}

#[test]
fn live_scope_requires_exact_current_unfenced_holder() {
    let mut scheduler = scheduler_with_epoch();
    let held_owner = owner(1);
    let held_set = set(&[(1, AccessMode::SharedRead), (2, AccessMode::Exclusive)]);
    let mut held = request(1, Priority::SystemAuth, 0);
    held.set = held_set.clone();

    assert!(!scheduler.holds_live_scope(lease(1), held_owner, &held_set));
    assert_eq!(
        scheduler.acquire(held, lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert!(scheduler.holds_live_scope(lease(1), held_owner, &held_set));
    assert!(!scheduler.holds_live_scope(lease(2), held_owner, &held_set));
    assert!(!scheduler.holds_live_scope(lease(1), owner(2), &held_set));
    assert!(!scheduler.holds_live_scope(
        lease(1),
        held_owner,
        &set(&[(1, AccessMode::SharedRead)])
    ));
    assert!(!scheduler.holds_live_scope(
        lease(1),
        held_owner,
        &set(&[(1, AccessMode::Exclusive), (2, AccessMode::Exclusive)])
    ));

    let held = scheduler
        .begin_quiescing(lease(1), held_owner, &held_set)
        .unwrap();
    assert!(!scheduler.holds_live_scope(lease(1), held_owner, &held_set));
    assert!(scheduler.quiescing_uncertain(held));
    assert!(!scheduler.holds_live_scope(lease(1), held_owner, &held_set));
}

#[test]
fn live_scope_does_not_survive_worker_death_or_epoch_change() {
    let mut scheduler = scheduler_with_epoch();
    let first = request(1, Priority::SystemAuth, 0);
    let held_owner = first.owner;
    let held_set = first.set.clone();
    assert_eq!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert!(scheduler.holds_live_scope(lease(1), held_owner, &held_set));
    assert_eq!(
        scheduler.fence_stopped_worker(held_owner.worker, DeviceEpoch::FIRST),
        1
    );
    assert!(!scheduler.holds_live_scope(lease(1), held_owner, &held_set));

    let mut other = scheduler_with_epoch();
    let next = request(2, Priority::SystemAuth, 0);
    let next_owner = next.owner;
    let next_set = next.set.clone();
    assert_eq!(
        other.acquire(next, lease(2)),
        GrantResult::Granted { lease: lease(2) }
    );
    other.set_epoch(DeviceEpoch::new(2).unwrap());
    assert!(!other.holds_live_scope(lease(2), next_owner, &next_set));
}

#[test]
fn cancel_queued_requires_exact_operation_and_lease_and_preserves_active_holder() {
    let mut scheduler = scheduler_with_epoch();
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Queued
    );
    assert!(!scheduler.cancel_queued(operation(1), lease(1)));
    assert!(!scheduler.cancel_queued(operation(1), lease(2)));
    assert!(!scheduler.cancel_queued(operation(2), lease(1)));
    assert_eq!(scheduler.queued(), 1);
    assert!(scheduler.cancel_queued(operation(2), lease(2)));
    assert!(!scheduler.cancel_queued(operation(2), lease(2)));
    assert_eq!(scheduler.queued(), 0);
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert!(scheduler.release(lease(1)).is_empty());
}

#[test]
fn stale_queued_request_can_be_cancelled_without_releasing_old_epoch_holder() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2));
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    assert!(scheduler.cancel_queued(operation(2), lease(2)));
    assert_eq!(scheduler.queued(), 0);
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert!(scheduler.dispatch().is_empty());
    let mut current = request(3, Priority::SystemAuth, 0);
    current.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(scheduler.acquire(current, lease(3)), GrantResult::Queued);
}

#[test]
fn expired_requests_are_returned_once_without_releasing_holders_or_current_queue() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    let old_request = request(2, Priority::SystemAuth, 1);
    assert_eq!(
        scheduler.acquire(old_request.clone(), lease(2)),
        GrantResult::Queued
    );
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    let mut current = request(3, Priority::SystemAuth, 2);
    current.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(scheduler.acquire(current, lease(3)), GrantResult::Queued);

    let expired = scheduler.take_expired_requests();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].request, old_request);
    assert_eq!(expired[0].lease, lease(2));
    assert_eq!(expired[0].epoch, DeviceEpoch::FIRST);
    assert!(scheduler.take_expired_requests().is_empty());
    assert_eq!(scheduler.queued(), 1);
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert!(scheduler.dispatch().is_empty());
    assert!(scheduler.cancel_queued(operation(3), lease(3)));
}

fn now(ns: u64) -> BootInstant {
    BootInstant::new(BootId::from_bytes([1u8; 16]).expect("валид"), ns)
}

fn request(op: u8, priority: Priority, ns: u64) -> Request {
    Request {
        owner: owner(op),
        priority,
        set: set(&[(1, AccessMode::Exclusive)]),
        since: now(ns),
        mutating: false,
    }
}

#[test]
fn starved_interactive_gets_next_slot_without_preempting_holder() {
    let mut scheduler = scheduler_with_epoch();
    let mut active = request(1, Priority::SystemAuth, 0);
    active.mutating = true;
    assert_eq!(
        scheduler.acquire(active, lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert_eq!(
        scheduler.acquire(request(2, Priority::InteractiveManage, 0), lease(2)),
        GrantResult::Queued
    );
    assert_eq!(
        scheduler.acquire(request(3, Priority::SystemAuth, 1), lease(3)),
        GrantResult::Queued
    );
    assert!(scheduler.dispatch_at(now(30_000_000_001)).is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert_eq!(
        scheduler.release_at(lease(1), now(30_000_000_001)),
        vec![(lease(2), operation(2))]
    );
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(2)));
    assert_eq!(scheduler.release(lease(2)), vec![(lease(3), operation(3))]);
}

#[test]
fn starvation_promotion_preserves_fifo_and_boot_boundary() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    scheduler.acquire(
        request(2, Priority::InteractiveManage, 30_000_000_000),
        lease(2),
    );
    scheduler.acquire(request(3, Priority::InteractiveManage, 0), lease(3));
    scheduler.acquire(request(4, Priority::SystemAuth, 0), lease(4));
    assert_eq!(
        scheduler.release_at(lease(1), now(30_000_000_001)),
        vec![(lease(4), operation(4))]
    );
    let other_boot = BootInstant::new(BootId::from_bytes([2; 16]).unwrap(), 90_000_000_000);
    assert_eq!(
        scheduler.release_at(lease(4), other_boot),
        vec![(lease(2), operation(2))]
    );
}

fn owner(op: u8) -> carosign_runtime::ports::ResourceOwner {
    carosign_runtime::ports::ResourceOwner {
        operation: operation(op),
        worker: carosign_core::ids::WorkerInstanceId::from_bytes([op; 16]).unwrap(),
        authenticator: carosign_core::device::AuthenticatorRef {
            id: carosign_core::ids::AuthenticatorId::from_bytes([7; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    }
}

fn scheduler_with_epoch() -> Scheduler {
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    scheduler
}

#[test]
fn recovered_resource_blocks_every_mode_and_survives_epoch_change() {
    let mut scheduler = Scheduler::new();
    scheduler.restore_fences([resource(1)]).unwrap();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    let mut independent = request(2, Priority::SystemAuth, 1);
    independent.set = set(&[(2, AccessMode::Exclusive)]);
    assert_eq!(
        scheduler.acquire(independent, lease(2)),
        GrantResult::Granted { lease: lease(2) }
    );
    let mut blocked = request(1, Priority::SystemAuth, 0);
    blocked.set = set(&[(1, AccessMode::SharedRead), (2, AccessMode::Exclusive)]);
    assert_eq!(scheduler.acquire(blocked, lease(1)), GrantResult::Queued);
    assert!(scheduler.restore_fences([resource(3)]).is_err());
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    let mut after_restart = request(3, Priority::SystemAuth, 2);
    after_restart.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(
        scheduler.acquire(after_restart, lease(3)),
        GrantResult::Queued
    );
    assert!(scheduler.dispatch().is_empty());
}

#[test]
fn stopped_worker_is_denied_before_and_after_queuing_and_new_epoch_is_distinct() {
    let mut scheduler = scheduler_with_epoch();
    let stopped = owner(2).worker;
    assert_eq!(
        scheduler.fence_stopped_worker(stopped, DeviceEpoch::FIRST),
        0
    );
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Denied
    );
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    let queued = request(3, Priority::SystemAuth, 0);
    assert_eq!(
        scheduler.acquire(queued.clone(), lease(3)),
        GrantResult::Queued
    );
    assert_eq!(
        scheduler.fence_stopped_worker(queued.owner.worker, DeviceEpoch::FIRST),
        0
    );
    assert!(scheduler.release(lease(1)).is_empty());
    let expired = scheduler.take_expired_requests();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].request, queued);
    assert!(scheduler.take_expired_requests().is_empty());
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    let mut next = request(2, Priority::SystemAuth, 0);
    next.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(
        scheduler.acquire(next, lease(4)),
        GrantResult::Granted { lease: lease(4) }
    );
}

#[test]
fn exhausted_stopped_worker_capacity_closes_all_new_grants() {
    let mut scheduler = scheduler_with_epoch();
    for n in 1u16..=257 {
        let mut bytes = [0u8; 16];
        bytes[..2].copy_from_slice(&n.to_be_bytes());
        let worker = carosign_core::ids::WorkerInstanceId::from_bytes(bytes).unwrap();
        scheduler.fence_stopped_worker(worker, DeviceEpoch::FIRST);
    }
    assert_eq!(
        scheduler.acquire(request(4, Priority::SystemAuth, 0), lease(4)),
        GrantResult::Denied
    );
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    let mut next = request(4, Priority::SystemAuth, 0);
    next.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(scheduler.acquire(next, lease(5)), GrantResult::Denied);
}

#[test]
fn quiescing_fences_shared_set_before_wait_and_rejects_foreign_scope() {
    use carosign_runtime::scheduler::FenceStage;
    let mut scheduler = scheduler_with_epoch();
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = set(&[(1, AccessMode::SharedRead), (2, AccessMode::Exclusive)]);
    let exact = first.set.clone();
    let actual_owner = first.owner;
    assert!(matches!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { .. }
    ));
    let mut foreign = actual_owner;
    foreign.worker = carosign_core::ids::WorkerInstanceId::from_bytes([9; 16]).unwrap();
    assert!(
        scheduler
            .begin_quiescing(lease(1), foreign, &exact)
            .is_none()
    );
    assert!(
        scheduler
            .begin_quiescing(
                lease(1),
                actual_owner,
                &set(&[(1, AccessMode::Exclusive), (2, AccessMode::Exclusive)])
            )
            .is_none()
    );
    assert!(
        scheduler
            .begin_quiescing(lease(1), actual_owner, &set(&[(1, AccessMode::SharedRead)]))
            .is_none()
    );
    let permit = scheduler
        .begin_quiescing(lease(1), actual_owner, &exact)
        .unwrap();
    assert!(
        scheduler
            .begin_quiescing(lease(1), actual_owner, &exact)
            .is_none()
    );
    assert_eq!(scheduler.fence_stage(lease(1)), Some(FenceStage::Quiescing));
    let mut reader = request(2, Priority::SystemAuth, 1);
    reader.set = set(&[(1, AccessMode::SharedRead)]);
    assert_eq!(scheduler.acquire(reader, lease(2)), GrantResult::Queued);
    assert!(scheduler.release(lease(1)).is_empty());
    assert!(scheduler.quiescing_uncertain(permit));
    assert_eq!(scheduler.fence_stage(lease(1)), Some(FenceStage::Uncertain));
    assert!(scheduler.dispatch().is_empty());
    assert!(scheduler.release(lease(1)).is_empty());
}

#[test]
fn lost_quiescing_holder_and_epoch_change_never_unfence() {
    use carosign_runtime::scheduler::FenceStage;
    let mut scheduler = scheduler_with_epoch();
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = set(&[(1, AccessMode::SharedRead)]);
    let held_set = first.set.clone();
    let owner = first.owner;
    scheduler.acquire(first, lease(1));
    drop(
        scheduler
            .begin_quiescing(lease(1), owner, &held_set)
            .unwrap(),
    );
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    assert_eq!(scheduler.fence_stage(lease(1)), Some(FenceStage::Quiescing));
    assert!(
        scheduler
            .begin_quiescing(lease(1), owner, &held_set)
            .is_none()
    );
    assert!(scheduler.release(lease(1)).is_empty());
    let mut other = request(2, Priority::SystemAuth, 1);
    other.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    other.set = set(&[(1, AccessMode::SharedRead)]);
    assert_eq!(scheduler.acquire(other, lease(2)), GrantResult::Queued);
}

#[test]
fn absent_device_epoch_never_grants_or_queues_resources() {
    let mut scheduler = Scheduler::new();
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Denied
    );
    assert_eq!(scheduler.queued(), 0);
    assert_eq!(scheduler.holder_of(resource(1)), None);
    assert!(scheduler.dispatch().is_empty());
    scheduler.set_epoch(DeviceEpoch::FIRST);
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
}

#[test]
fn queued_request_is_not_reissued_in_a_different_device_epoch() {
    let mut scheduler = scheduler_with_epoch();
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Queued
    );
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    // The old holder remains: the change of era does not confirm the rest of the resource.
    assert!(scheduler.release(lease(1)).is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert_eq!(scheduler.queued(), 1);
    assert!(scheduler.dispatch().is_empty());
    assert_eq!(
        scheduler.acquire(request(3, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Denied
    );
}

#[test]
fn stale_shared_holder_blocks_new_epoch_until_recovery() {
    let mut scheduler = scheduler_with_epoch();
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = set(&[(1, AccessMode::SharedRead)]);
    let mut second = request(2, Priority::SystemAuth, 0);
    second.set = set(&[(1, AccessMode::SharedRead)]);
    assert_eq!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    second.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(scheduler.acquire(second, lease(2)), GrantResult::Queued);
    assert!(scheduler.release(lease(1)).is_empty());
    assert!(scheduler.dispatch().is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
}

#[test]
fn stale_epoch_update_does_not_reactivate_old_leases_or_requests() {
    let mut scheduler = scheduler_with_epoch();
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Queued
    );
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    scheduler.set_epoch(DeviceEpoch::FIRST);
    assert!(scheduler.release(lease(1)).is_empty());
    assert!(scheduler.dispatch().is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert_eq!(scheduler.queued(), 1);
}

#[test]
fn exclusive_resource_is_granted_once() {
    let mut scheduler = scheduler_with_epoch();
    assert!(matches!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { .. }
    ));
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Queued
    );
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
}

#[test]
fn same_operation_cannot_hold_or_queue_multiple_leases() {
    let mut scheduler = scheduler_with_epoch();
    assert_eq!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    let mut shared = request(1, Priority::SystemAuth, 1);
    shared.set = set(&[(1, AccessMode::SharedRead)]);
    assert_eq!(scheduler.acquire(shared, lease(2)), GrantResult::Denied);
    let mut independent = request(1, Priority::SystemAuth, 2);
    independent.set = set(&[(2, AccessMode::Exclusive)]);
    assert_eq!(
        scheduler.acquire(independent, lease(3)),
        GrantResult::Denied
    );
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 3), lease(4)),
        GrantResult::Queued,
    );
    let mut queued_again = request(2, Priority::SystemAuth, 4);
    queued_again.set = set(&[(2, AccessMode::Exclusive)]);
    assert_eq!(
        scheduler.acquire(queued_again, lease(5)),
        GrantResult::Denied
    );
    assert_eq!(scheduler.release(lease(1)), vec![(lease(4), operation(2))]);
    assert_eq!(scheduler.holder_of(resource(2)), None);
}

#[test]
fn uncertain_shared_lease_blocks_new_users_and_plain_release() {
    let mut scheduler = scheduler_with_epoch();
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = set(&[(1, AccessMode::SharedRead), (2, AccessMode::SharedRead)]);
    let full_set = first.set.clone();
    let full_owner = first.owner;
    assert!(matches!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { .. }
    ));
    assert!(!scheduler.mark_uncertain_scope(lease(1), owner(2), &full_set));
    let mut stale = full_owner;
    stale.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert!(!scheduler.mark_uncertain_scope(lease(1), stale, &full_set));
    assert!(scheduler.mark_uncertain_scope(lease(1), full_owner, &full_set));
    assert!(scheduler.mark_uncertain_scope(lease(1), full_owner, &full_set));

    let mut second = request(2, Priority::SystemAuth, 1);
    second.set = set(&[(2, AccessMode::SharedRead)]);
    assert_eq!(scheduler.acquire(second, lease(2)), GrantResult::Queued);
    assert!(scheduler.release(lease(1)).is_empty());
    assert!(scheduler.dispatch().is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert_eq!(scheduler.holder_of(resource(2)), Some(operation(1)));

    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    assert!(scheduler.release(lease(1)).is_empty());
    let mut third = request(3, Priority::SystemAuth, 2);
    third.set = set(&[(2, AccessMode::SharedRead)]);
    third.owner.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    assert_eq!(scheduler.acquire(third, lease(3)), GrantResult::Queued);
    assert!(scheduler.dispatch().is_empty());
}

#[test]
fn stopped_worker_fences_every_matching_lease_and_invalidates_cleanup_holder() {
    use carosign_runtime::scheduler::FenceStage;

    let mut scheduler = scheduler_with_epoch();
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = set(&[(1, AccessMode::SharedRead), (2, AccessMode::SharedRead)]);
    let first_set = first.set.clone();
    let first_owner = first.owner;
    let mut second = request(2, Priority::SystemAuth, 1);
    second.set = set(&[(1, AccessMode::SharedRead)]);
    assert!(matches!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        scheduler.acquire(second, lease(2)),
        GrantResult::Granted { .. }
    ));
    let held = scheduler
        .begin_quiescing(lease(1), first_owner, &first_set)
        .unwrap();

    assert_eq!(
        scheduler.fence_stopped_worker(first_owner.worker, DeviceEpoch::new(2).unwrap()),
        0
    );
    assert_eq!(scheduler.fence_stage(lease(1)), Some(FenceStage::Quiescing));
    assert_eq!(
        scheduler.fence_stopped_worker(first_owner.worker, DeviceEpoch::FIRST),
        1
    );
    assert_eq!(scheduler.fence_stage(lease(1)), Some(FenceStage::Uncertain));
    assert_eq!(scheduler.fence_stage(lease(2)), None);
    assert!(!scheduler.quiescing_uncertain(held));
    assert!(scheduler.release(lease(1)).is_empty());

    let mut later = request(3, Priority::SystemAuth, 2);
    later.set = set(&[(2, AccessMode::SharedRead)]);
    assert_eq!(scheduler.acquire(later, lease(3)), GrantResult::Queued);
    assert!(scheduler.release(lease(2)).is_empty());
    assert!(scheduler.dispatch().is_empty());
    assert_eq!(scheduler.holder_of(resource(2)), Some(operation(1)));
}

#[test]
fn active_lease_id_cannot_be_reused_for_another_resource() {
    let mut scheduler = scheduler_with_epoch();
    assert!(matches!(
        scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1)),
        GrantResult::Granted { .. }
    ));
    let mut other = request(2, Priority::SystemAuth, 0);
    other.set = set(&[(2, AccessMode::Exclusive)]);
    assert_eq!(scheduler.acquire(other, lease(1)), GrantResult::Denied);
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(1)));
    assert_eq!(scheduler.holder_of(resource(2)), None);
}

#[test]
fn queued_dispatch_keeps_the_requested_identifier() {
    let mut scheduler = scheduler_with_epoch();
    let mut id = [0; 16];
    id[..8].copy_from_slice(&1u64.to_be_bytes());
    id[15] = 1;
    let collision = LeaseId::from_bytes(id).unwrap();
    let mut other = request(2, Priority::SystemAuth, 0);
    other.set = set(&[(2, AccessMode::Exclusive)]);
    assert!(matches!(
        scheduler.acquire(other, collision),
        GrantResult::Granted { .. }
    ));
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    scheduler.acquire(request(3, Priority::SystemAuth, 0), lease(3));
    let granted = scheduler.release(lease(1));
    assert_eq!(granted.len(), 1);
    assert_eq!(granted[0].0, lease(3));
    scheduler.release(granted[0].0);
    assert_eq!(scheduler.holder_of(resource(2)), Some(operation(2)));
}

#[test]
fn queued_lease_cannot_be_reused_before_dispatch() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    assert_eq!(
        scheduler.acquire(request(2, Priority::SystemAuth, 0), lease(2)),
        GrantResult::Queued
    );
    let mut independent = request(3, Priority::SystemAuth, 0);
    independent.set = set(&[(2, AccessMode::Exclusive)]);
    assert_eq!(
        scheduler.acquire(independent, lease(2)),
        GrantResult::Denied
    );
    assert_eq!(scheduler.holder_of(resource(2)), None);
    assert_eq!(scheduler.release(lease(1)), vec![(lease(2), operation(2))]);
}

#[test]
fn whole_set_is_granted_atomically_or_not_at_all() {
    let mut scheduler = scheduler_with_epoch();
    let big = set(&[(1, AccessMode::Exclusive), (2, AccessMode::Exclusive)]);
    scheduler.acquire(
        Request {
            owner: owner(9),
            priority: Priority::SystemAuth,
            set: set(&[(2, AccessMode::Exclusive)]),
            since: now(0),
            mutating: false,
        },
        lease(2),
    );
    let result = scheduler.acquire(
        Request {
            owner: owner(1),
            priority: Priority::SystemAuth,
            set: big,
            since: now(0),
            mutating: false,
        },
        lease(1),
    );
    assert_eq!(result, GrantResult::Queued);
    // None of the recruitment resources have been issued.
    assert_eq!(scheduler.holder_of(resource(1)), None);
}

#[test]
fn release_hands_the_slot_to_the_queue() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    scheduler.acquire(request(2, Priority::Diagnostic, 0), lease(2));
    assert_eq!(scheduler.queued(), 1);
    let granted = scheduler.release(lease(1));
    assert_eq!(granted.len(), 1);
    assert_eq!(granted[0].1, operation(2));
    assert_eq!(scheduler.queued(), 0);
}

#[test]
fn higher_class_goes_first() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    scheduler.acquire(request(2, Priority::Diagnostic, 10), lease(2));
    scheduler.acquire(request(3, Priority::InteractiveManage, 20), lease(3));
    socket_wait(&mut scheduler);
    let granted = scheduler.release(lease(1));
    // `InteractiveManage` higher `Diagnostic`despite the late arrival.
    assert_eq!(granted[0].1, operation(3));
}

#[test]
fn fifo_within_priority_follows_enqueue_order_not_timestamps() {
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    scheduler.acquire(request(2, Priority::Diagnostic, 20), lease(2));
    scheduler.acquire(request(3, Priority::Diagnostic, 10), lease(3));
    scheduler.acquire(request(4, Priority::Diagnostic, 10), lease(4));

    assert_eq!(scheduler.release(lease(1)), vec![(lease(2), operation(2))]);
    assert_eq!(scheduler.release(lease(2)), vec![(lease(3), operation(3))]);
    assert_eq!(scheduler.release(lease(3)), vec![(lease(4), operation(4))]);
}

#[test]
fn fifo_does_not_skip_blocked_multiresource_request_in_same_class() {
    let mut scheduler = scheduler_with_epoch();
    assert!(matches!(
        scheduler.acquire(request(1, Priority::Diagnostic, 0), lease(1)),
        GrantResult::Granted { .. }
    ));
    let mut first = request(2, Priority::SystemAuth, 1);
    first.set = set(&[(1, AccessMode::Exclusive), (2, AccessMode::Exclusive)]);
    assert_eq!(scheduler.acquire(first, lease(2)), GrantResult::Queued);
    let mut later = request(3, Priority::SystemAuth, 2);
    later.set = set(&[(3, AccessMode::Exclusive)]);
    assert_eq!(scheduler.acquire(later, lease(3)), GrantResult::Queued);
    assert!(scheduler.dispatch().is_empty());
    assert_eq!(scheduler.holder_of(resource(3)), None);
    assert_eq!(
        scheduler.release(lease(1)),
        vec![(lease(2), operation(2)), (lease(3), operation(3))]
    );
}

fn socket_wait(scheduler: &mut Scheduler) {
    // We expect nothing: order is given by classes and time, not by hours.
    let _ = scheduler.dispatch();
}

#[test]
fn queue_is_bounded() {
    // The baseline limit of document 13 is 16, and 64 is only a ceiling for the separately
    // considered configuration.
    assert_eq!(QUEUE_MAX, 16);
    let mut scheduler = scheduler_with_epoch();
    scheduler.acquire(request(1, Priority::SystemAuth, 0), lease(1));
    for i in 0..QUEUE_MAX as u8 {
        let result = scheduler.acquire(request(i + 2, Priority::Diagnostic, 0), lease(i + 2));
        // All available seats are occupied with waiting requests.
        assert!(matches!(result, GrantResult::Queued));
    }
    assert_eq!(scheduler.queued(), QUEUE_MAX);
    assert_eq!(
        scheduler.acquire(request(100, Priority::Diagnostic, 0), lease(100)),
        GrantResult::Denied
    );
    assert_eq!(scheduler.queued(), QUEUE_MAX);
}

#[test]
fn starvation_is_measured_not_guessed() {
    let request = request(1, Priority::InteractiveManage, 0);
    assert!(!is_starving(now(30_000_000_000 - 1), &request));
    assert!(!is_starving(now(30_000_000_000), &request));
    assert!(is_starving(now(30_000_000_001), &request));
    let other_boot = BootInstant::new(BootId::from_bytes([2; 16]).unwrap(), 30_000_000_001);
    assert!(!is_starving(other_boot, &request));
    let mut diagnostic = request.clone();
    diagnostic.priority = Priority::Diagnostic;
    assert!(!is_starving(now(30_000_000_001), &diagnostic));
    let mut system_auth = request;
    system_auth.priority = Priority::SystemAuth;
    assert!(!is_starving(now(30_000_000_001), &system_auth));
}

#[test]
fn exact_uncertain_scope_rejects_partial_extra_duplicate_and_foreign_lease() {
    let mut scheduler = scheduler_with_epoch();
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = set(&[(1, AccessMode::SharedRead), (2, AccessMode::SharedRead)]);
    assert_eq!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    let mut second = request(2, Priority::SystemAuth, 1);
    second.set = set(&[(1, AccessMode::SharedRead)]);
    assert_eq!(
        scheduler.acquire(second, lease(2)),
        GrantResult::Granted { lease: lease(2) }
    );
    let partial = set(&[(1, AccessMode::SharedRead)]);
    let extra = set(&[
        (1, AccessMode::SharedRead),
        (2, AccessMode::SharedRead),
        (3, AccessMode::SharedRead),
    ]);
    let full = set(&[(1, AccessMode::SharedRead), (2, AccessMode::SharedRead)]);
    let wrong_mode = set(&[(1, AccessMode::Exclusive), (2, AccessMode::SharedRead)]);
    let mut stale = owner(1);
    stale.authenticator.device_epoch = DeviceEpoch::new(2).unwrap();
    for (candidate_lease, candidate_owner, resources) in [
        (lease(1), owner(1), &partial),
        (lease(1), owner(1), &extra),
        (lease(1), owner(1), &wrong_mode),
        (lease(2), owner(1), &partial),
        (lease(1), stale, &full),
        (lease(1), owner(2), &full),
    ] {
        assert!(!scheduler.mark_uncertain_scope(candidate_lease, candidate_owner, resources));
    }
    assert!(
        ResourceSet::new(vec![
            ResourceClaim {
                id: resource(1),
                mode: AccessMode::SharedRead
            },
            ResourceClaim {
                id: resource(1),
                mode: AccessMode::SharedRead
            }
        ])
        .is_err()
    );
    // The refusal did not change any extraditions and left no hidden quarantine.
    assert!(scheduler.release(lease(2)).is_empty());
    assert!(scheduler.release(lease(1)).is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), None);
    assert_eq!(scheduler.holder_of(resource(2)), None);
}

#[test]
fn shared_read_allows_two_readers() {
    let mut scheduler = scheduler_with_epoch();
    let shared = set(&[(1, AccessMode::SharedRead)]);
    let mut first = request(1, Priority::SystemAuth, 0);
    first.set = shared.clone();
    let mut second = request(2, Priority::SystemAuth, 0);
    second.set = shared;
    assert!(matches!(
        scheduler.acquire(first, lease(1)),
        GrantResult::Granted { .. }
    ));
    assert!(matches!(
        scheduler.acquire(second, lease(2)),
        GrantResult::Granted { .. }
    ));
    assert!(scheduler.release(lease(1)).is_empty());
    assert_eq!(scheduler.holder_of(resource(1)), Some(operation(2)));
    assert_eq!(
        scheduler.acquire(request(3, Priority::SystemAuth, 0), lease(3)),
        GrantResult::Queued
    );
    let dispatched = scheduler.release(lease(2));
    assert_eq!(dispatched.len(), 1);
    assert_eq!(dispatched[0].1, operation(3));
}

#[test]
fn owner_detach_does_not_cancel_by_itself() {
    let mut owner = OwnerHandle::new(CallerContext::from_transport(
        [3u8; 32],
        1000,
        TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
    ));
    assert_eq!(owner.channel(), OwnerChannel::Attached);
    owner.detach();
    assert!(!owner.may_receive());
    // ⚠️ The blackout mark is not a cancellation: the actor makes the decision.
}

#[test]
fn observers_are_bounded() {
    let mut observers = Observers::default();
    for id in 0..OBSERVERS_MAX as u32 {
        observers
            .subscribe(Observer {
                id,
                after_sequence: 0,
            })
            .expect("влезает");
    }
    assert!(
        observers
            .subscribe(Observer {
                id: 100,
                after_sequence: 0
            })
            .is_err()
    );
}

#[test]
fn event_gap_is_explicit() {
    let state = carosign_engine::tests_support::initial();
    let mut stream = EventStream::new(state.common.context.operation_id);
    for _ in 0..(HISTORY_MAX + 2) {
        stream
            .publish(
                carosign_engine::OperationPhase::Running,
                EventKind::Progress(carosign_runtime::operation::events::Progress::Working),
            )
            .unwrap();
    }
    let replay = stream.replay_from(0, &state).unwrap();
    assert!(
        matches!(replay, carosign_runtime::operation::events::Replay::Gap { snapshot, last_sequence, .. } if snapshot.last_sequence == last_sequence)
    );
}

#[test]
fn authenticator_epochs_do_not_invalidate_other_devices() {
    use carosign_core::ids::AuthenticatorId;

    let mut scheduler = Scheduler::new();
    let first = request(1, Priority::SystemAuth, 0);
    let mut second = request(2, Priority::SystemAuth, 0);
    second.owner.authenticator.id = AuthenticatorId::from_bytes([73; 16]).unwrap();
    second.owner.authenticator.device_epoch = DeviceEpoch::new(7).unwrap();
    second.set = set(&[(73, AccessMode::Exclusive)]);
    scheduler.set_authenticator_epoch(first.owner.authenticator);
    scheduler.set_authenticator_epoch(second.owner.authenticator);
    assert_eq!(
        scheduler.acquire(first.clone(), lease(1)),
        GrantResult::Granted { lease: lease(1) }
    );
    assert_eq!(
        scheduler.acquire(second.clone(), lease(2)),
        GrantResult::Granted { lease: lease(2) }
    );

    let mut replacement = second.owner.authenticator;
    replacement.device_epoch = replacement.device_epoch.next().unwrap();
    scheduler.set_authenticator_epoch(replacement);
    assert!(scheduler.holds_live_scope(lease(1), first.owner, &first.set));
    assert!(!scheduler.holds_live_scope(lease(2), second.owner, &second.set));
    assert_eq!(
        scheduler.holder_of(resource(73)),
        Some(second.owner.operation)
    );
}
