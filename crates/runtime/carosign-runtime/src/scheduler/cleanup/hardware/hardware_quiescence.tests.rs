use super::super::*;
use super::*;
use crate::ports::{ResourceLease, ResourceOwner};
use carosign_core::{
    device::{AuthenticatorRef, DeviceEpoch},
    ids::{AuthenticatorId, LeaseId, OperationId, ResourceId, WorkerInstanceId},
    resource::{AccessMode, ResourceClaim, ResourceSet},
};

fn scope(lease: u8, epoch: u64, resource: u8) -> CleanupScope {
    CleanupScope::of(&ResourceLease::issued(
        LeaseId::from_bytes([lease; 16]).unwrap(),
        ResourceOwner {
            operation: OperationId::from_bytes([2; 16]).unwrap(),
            worker: WorkerInstanceId::from_bytes([3; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
                device_epoch: DeviceEpoch::new(epoch).unwrap(),
            },
        },
        ResourceSet::new(vec![ResourceClaim {
            id: ResourceId::from_bytes([resource; 16]).unwrap(),
            mode: AccessMode::Exclusive,
        }])
        .unwrap(),
    ))
}

fn observed(scope: CleanupScope, stage: Stage) -> Observation {
    // SAFETY: The fixture supplies a mock source for the model test and has no hardware access.
    unsafe { Observation::from_trusted_hardware(scope, stage) }
}

#[test]
fn refuses_skips_repeats_and_other_scope_without_advancing() {
    let wanted = scope(1, 1, 5);
    let mut state = HardwareQuiescence::new(wanted.clone());
    assert!(
        state
            .advance(observed(wanted.clone(), Stage::CallsDrained))
            .is_err()
    );
    for wrong in [scope(6, 1, 5), scope(1, 2, 5), scope(1, 1, 6)] {
        assert!(
            state
                .advance(observed(wrong, Stage::AdmissionsClosed))
                .is_err()
        );
    }
    assert_eq!(state.stage(), Stage::Bound);
    for step in [
        Stage::AdmissionsClosed,
        Stage::InterruptsDrained,
        Stage::CallsDrained,
        Stage::DeviceQuiesced,
    ] {
        assert!(state.advance(observed(wanted.clone(), step)).is_ok());
        assert!(state.advance(observed(wanted.clone(), step)).is_err());
        assert_eq!(state.stage(), step);
    }
}

#[test]
fn durable_fence_requires_complete_hardware_and_broker_release() {
    use crate::scheduler::cleanup::{CleanupPortFailure, TrustedCleanupBroker, confirm_released};
    use crate::scheduler::{GrantResult, Priority, Request, Scheduler};
    use carosign_core::{ids::BootId, operation::context::BootInstant};
    use carosign_persistence::redb::backend::InitialFenceStore;

    struct Broker(bool);
    impl TrustedCleanupBroker for Broker {
        fn quiesce<'a>(
            &'a self,
            _: &'a CleanupScope,
        ) -> crate::ports::BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
            Box::pin(async { Err(CleanupPortFailure::Uncertain) })
        }
        fn release<'a>(
            &'a self,
            scope: &'a CleanupScope,
            _: QuiescedLease,
        ) -> crate::ports::BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
            Box::pin(async move {
                if self.0 {
                    Ok(scope.clone())
                } else {
                    Err(CleanupPortFailure::Uncertain)
                }
            })
        }
    }

    let wanted = scope(1, 1, 5);
    let path = std::env::temp_dir().join(format!(
        "carosign-hw-fence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = std::sync::Arc::new(InitialFenceStore::create_new(&path).unwrap());
    let mut scheduler = Scheduler::new();
    scheduler
        .restore_durable_fences(store.fences().unwrap(), store.clone())
        .unwrap();
    scheduler.set_epoch(wanted.owner().authenticator.device_epoch);
    let result = scheduler.acquire(
        Request {
            owner: wanted.owner(),
            priority: Priority::SystemAuth,
            set: wanted.resource_set().clone(),
            since: BootInstant::new(BootId::from_bytes([9; 16]).unwrap(), 0),
            mutating: false,
        },
        wanted.lease_id(),
    );
    assert_eq!(
        result,
        GrantResult::Granted {
            lease: wanted.lease_id()
        }
    );
    scheduler
        .prepare_durable_fence(wanted.lease_id(), wanted.owner(), wanted.resource_set())
        .unwrap();
    let held = scheduler
        .begin_quiescing(wanted.lease_id(), wanted.owner(), wanted.resource_set())
        .unwrap();
    assert!(
        HardwareQuiescence::new(wanted.clone())
            .confirm_for(&held)
            .is_err()
    );
    let mut wrong = HardwareQuiescence::new(scope(6, 1, 5));
    let other = scope(6, 1, 5);
    for step in [
        Stage::AdmissionsClosed,
        Stage::InterruptsDrained,
        Stage::CallsDrained,
        Stage::DeviceQuiesced,
    ] {
        assert!(wrong.advance(observed(other.clone(), step)).is_ok());
    }
    assert!(wrong.confirm_for(&held).is_err());
    assert_eq!(store.fences().unwrap(), wanted.resources());

    let mut complete = HardwareQuiescence::new(wanted.clone());
    for step in [
        Stage::AdmissionsClosed,
        Stage::InterruptsDrained,
        Stage::CallsDrained,
        Stage::DeviceQuiesced,
    ] {
        assert!(complete.advance(observed(wanted.clone(), step)).is_ok());
    }
    let proof = complete.confirm_for(&held).unwrap();
    assert!(futures_executor::block_on(confirm_released(&Broker(false), &wanted, proof)).is_err());
    assert_eq!(store.fences().unwrap(), wanted.resources());

    let mut retry = HardwareQuiescence::new(wanted.clone());
    for step in [
        Stage::AdmissionsClosed,
        Stage::InterruptsDrained,
        Stage::CallsDrained,
        Stage::DeviceQuiesced,
    ] {
        assert!(retry.advance(observed(wanted.clone(), step)).is_ok());
    }
    let proof = retry.confirm_for(&held).unwrap();
    let released =
        futures_executor::block_on(confirm_released(&Broker(true), &wanted, proof)).unwrap();
    assert!(scheduler.finish_quiescing(held, released).is_ok());
    assert!(store.fences().unwrap().is_empty());
    drop(scheduler);
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert!(reopened.fences().unwrap().is_empty());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
