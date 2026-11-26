use super::super::*;
use crate::ports::{BoxFuture, ResourceLease, ResourceOwner};
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::ids::{AuthenticatorId, OperationId, WorkerInstanceId};
use carosign_core::ids::{LeaseId, ResourceId};
use carosign_core::resource::ResourceSet;
use carosign_core::resource::{AccessMode, ResourceClaim};
use carosign_persistence::redb::backend::{FenceScope, InitialFenceStore};

#[test]
fn armed_binding_requires_durable_cleanup_before_live_release() {
    use crate::scheduler::{GrantResult, Priority, Request, Scheduler};
    use carosign_core::ids::BootId;
    use carosign_core::operation::context::BootInstant;

    let path = std::env::temp_dir().join(format!(
        "carosign-binding-fence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = std::sync::Arc::new(InitialFenceStore::create_new(&path).unwrap());
    let resource = ResourceId::from_bytes([41; 16]).unwrap();
    let set = ResourceSet::new(vec![ResourceClaim {
        id: resource,
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([42; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([43; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([44; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = LeaseId::from_bytes([45; 16]).unwrap();
    let mut scheduler = Scheduler::new();
    scheduler
        .restore_durable_fences(store.fences().unwrap(), store.clone())
        .unwrap();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    assert_eq!(
        scheduler.acquire(
            Request {
                owner,
                priority: Priority::SystemAuth,
                set: set.clone(),
                since: BootInstant::new(BootId::from_bytes([46; 16]).unwrap(), 0),
                mutating: false,
            },
            lease
        ),
        GrantResult::Granted { lease }
    );
    scheduler.prepare_durable_fence(lease, owner, &set).unwrap();
    assert_eq!(store.fences().unwrap(), vec![resource]);
    assert!(scheduler.prepare_durable_fence(lease, owner, &set).is_err());
    scheduler.mark_processes_exited(owner.worker);
    assert!(!scheduler.release_after_processes_exit(lease, owner, &set));
    scheduler.release(lease);
    assert!(scheduler.holds_scope(lease, owner, &set));
    let held = scheduler.begin_quiescing(lease, owner, &set).unwrap();
    // Even another file with an identical record is not the owner of the issue.
    let other_path = path.with_extension("other");
    let other = InitialFenceStore::create_new(&other_path).unwrap();
    other
        .fence_scoped(&FenceScope {
            lease,
            operation: owner.operation,
            worker: owner.worker,
            authenticator: owner.authenticator,
            resources: set.clone(),
        })
        .unwrap();
    let wrong_proof = Released {
        scope: CleanupScope::of(&ResourceLease::issued(lease, owner, set.clone())),
        fence: Some(held.key.clone()),
    };
    let held = match scheduler.finish_quiescing_durable(held, wrong_proof, &other) {
        Err(held) => held,
        Ok(()) => panic!("чужое хранилище освободило выдачу"),
    };
    assert_eq!(store.fences().unwrap(), vec![resource]);
    drop(other);
    std::fs::remove_file(other_path).unwrap();
    let proof = Released {
        scope: CleanupScope::of(&ResourceLease::issued(lease, owner, set.clone())),
        fence: Some(held.key.clone()),
    };
    assert!(scheduler.finish_quiescing(held, proof).is_ok());
    assert!(!scheduler.holds_scope(lease, owner, &set));
    assert!(store.fences().unwrap().is_empty());
    drop(scheduler);
    drop(store);
    assert!(
        InitialFenceStore::open_guarded(&path)
            .unwrap()
            .fences()
            .unwrap()
            .is_empty()
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn durable_release_requires_exact_proof_and_survives_reopen() {
    let path = std::env::temp_dir().join(format!(
        "carosign-fence-release-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let resources = ResourceSet::new(
        vec![1, 2]
            .into_iter()
            .map(|byte| ResourceClaim {
                id: ResourceId::from_bytes([byte; 16]).unwrap(),
                mode: AccessMode::Exclusive,
            })
            .collect(),
    )
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([4; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([5; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = LeaseId::from_bytes([6; 16]).unwrap();
    let held = crate::scheduler::Quiescing {
        lease,
        owner,
        set: resources.clone(),
        key: std::sync::Arc::new(()),
    };
    let scope = CleanupScope::of(&ResourceLease::issued(lease, owner, resources.clone()));
    let persisted = FenceScope {
        lease,
        operation: owner.operation,
        worker: owner.worker,
        authenticator: owner.authenticator,
        resources: resources.clone(),
    };
    let store = InitialFenceStore::create_new(&path).unwrap();
    store.fence_scoped(&persisted).unwrap();

    // Matching fields without a non-repeatable key does not confirm cleaning.
    let foreign = Released {
        scope: scope.clone(),
        fence: Some(std::sync::Arc::new(())),
    };
    assert!(foreign.release_durable_fence(&store, &held).is_err());
    assert_eq!(store.fences().unwrap().len(), 2);
    let unbound = Released {
        scope: scope.clone(),
        fence: None,
    };
    assert!(unbound.release_durable_fence(&store, &held).is_err());
    assert_eq!(store.fences().unwrap().len(), 2);

    // The key to another full area does not allow release either.
    let other = CleanupScope::of(&ResourceLease::issued(
        LeaseId::from_bytes([7; 16]).unwrap(),
        owner,
        resources.clone(),
    ));
    let mismatched = Released {
        scope: other,
        fence: Some(held.key.clone()),
    };
    assert!(mismatched.release_durable_fence(&store, &held).is_err());
    assert_eq!(store.fences().unwrap().len(), 2);

    // Loss of a trusted path prior to fixation does not remove records and proof cannot be
    // reused after an indefinite error.
    let moved = path.with_extension("moved");
    std::fs::rename(&path, &moved).unwrap();
    let uncertain = Released {
        scope: scope.clone(),
        fence: Some(held.key.clone()),
    };
    assert!(uncertain.release_durable_fence(&store, &held).is_err());
    std::fs::rename(&moved, &path).unwrap();
    assert_eq!(store.fences().unwrap().len(), 2);

    let released = Released {
        scope: scope.clone(),
        fence: Some(held.key.clone()),
    };
    let released = released.release_durable_fence(&store, &held).ok().unwrap();
    assert!(store.fences().unwrap().is_empty());
    // Repeating the same affirmation does not create another successful outcome.
    assert!(released.release_durable_fence(&store, &held).is_err());
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert!(reopened.fences().unwrap().is_empty());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn durable_release_rejects_store_not_bound_to_live_lease() {
    use crate::scheduler::{GrantResult, Priority, Request, Scheduler};
    use carosign_core::ids::BootId;
    use carosign_core::operation::context::BootInstant;

    let path = std::env::temp_dir().join(format!(
        "carosign-foreign-durable-release-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let resource = ResourceId::from_bytes([31; 16]).unwrap();
    let set = ResourceSet::new(vec![ResourceClaim {
        id: resource,
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([32; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([33; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([34; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = LeaseId::from_bytes([35; 16]).unwrap();
    let store = InitialFenceStore::create_new(&path).unwrap();
    store
        .fence_scoped(&FenceScope {
            lease,
            operation: owner.operation,
            worker: owner.worker,
            authenticator: owner.authenticator,
            resources: set.clone(),
        })
        .unwrap();
    // Matching identifiers do not link the record to live delivery.
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    assert_eq!(
        scheduler.acquire(
            Request {
                owner,
                priority: Priority::SystemAuth,
                set: set.clone(),
                since: BootInstant::new(BootId::from_bytes([36; 16]).unwrap(), 0),
                mutating: false,
            },
            lease,
        ),
        GrantResult::Granted { lease }
    );
    let held = scheduler.begin_quiescing(lease, owner, &set).unwrap();
    let proof = Released {
        scope: CleanupScope::of(&ResourceLease::issued(lease, owner, set.clone())),
        fence: Some(held.key.clone()),
    };
    assert!(
        scheduler
            .finish_quiescing_durable(held, proof, &store)
            .is_err()
    );
    assert!(scheduler.holds_scope(lease, owner, &set));
    assert_eq!(store.fences().unwrap(), vec![resource]);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn durable_release_rechecks_scheduler_epoch_before_changing_disk() {
    use crate::scheduler::{GrantResult, Priority, Request, Scheduler};
    use carosign_core::ids::BootId;
    use carosign_core::operation::context::BootInstant;

    let path = std::env::temp_dir().join(format!("carosign-stale-release-{}", std::process::id()));
    let resource = ResourceId::from_bytes([11; 16]).unwrap();
    let set = ResourceSet::new(vec![ResourceClaim {
        id: resource,
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([12; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([13; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([14; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = LeaseId::from_bytes([15; 16]).unwrap();
    let mut scheduler = Scheduler::new();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    let request = Request {
        owner,
        priority: Priority::SystemAuth,
        set: set.clone(),
        since: BootInstant::new(BootId::from_bytes([16; 16]).unwrap(), 0),
        mutating: false,
    };
    assert_eq!(
        scheduler.acquire(request, lease),
        GrantResult::Granted { lease }
    );
    let held = scheduler.begin_quiescing(lease, owner, &set).unwrap();
    let scope = CleanupScope::of(&ResourceLease::issued(lease, owner, set.clone()));
    let proof = Released {
        scope,
        fence: Some(held.key.clone()),
    };
    let store = InitialFenceStore::create_new(&path).unwrap();
    store
        .fence_scoped(&FenceScope {
            lease,
            operation: owner.operation,
            worker: owner.worker,
            authenticator: owner.authenticator,
            resources: set,
        })
        .unwrap();
    scheduler.set_epoch(DeviceEpoch::new(2).unwrap());
    assert!(
        scheduler
            .finish_quiescing_durable(held, proof, &store)
            .is_err()
    );
    assert_eq!(store.fences().unwrap(), vec![resource]);
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(reopened.fences().unwrap(), vec![resource]);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn confirmed_cleanup_releases_matching_durable_and_live_lease() {
    use crate::scheduler::{GrantResult, Priority, Request, Scheduler};
    use carosign_core::ids::BootId;
    use carosign_core::operation::context::BootInstant;

    struct ConfirmedBroker;
    impl TrustedCleanupBroker for ConfirmedBroker {
        fn quiesce<'a>(
            &'a self,
            scope: &'a CleanupScope,
        ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
            Box::pin(async move { Ok(scope.clone()) })
        }
        fn release<'a>(
            &'a self,
            scope: &'a CleanupScope,
            proof: QuiescedLease,
        ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>> {
            Box::pin(async move {
                if proof.scope() != scope {
                    return Err(CleanupPortFailure::Uncertain);
                }
                Ok(scope.clone())
            })
        }
    }

    let path = std::env::temp_dir().join(format!(
        "carosign-confirmed-release-{}",
        std::process::id()
    ));
    let resource = ResourceId::from_bytes([21; 16]).unwrap();
    let set = ResourceSet::new(vec![ResourceClaim {
        id: resource,
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([22; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([23; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([24; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = LeaseId::from_bytes([25; 16]).unwrap();
    let store = std::sync::Arc::new(InitialFenceStore::create_new(&path).unwrap());
    let mut scheduler = Scheduler::new();
    scheduler
        .restore_durable_fences(store.fences().unwrap(), store.clone())
        .unwrap();
    scheduler.set_epoch(DeviceEpoch::FIRST);
    let request = Request {
        owner,
        priority: Priority::SystemAuth,
        set: set.clone(),
        since: BootInstant::new(BootId::from_bytes([26; 16]).unwrap(), 0),
        mutating: false,
    };
    assert_eq!(
        scheduler.acquire(request, lease),
        GrantResult::Granted { lease }
    );
    scheduler.prepare_durable_fence(lease, owner, &set).unwrap();
    let held = scheduler.begin_quiescing(lease, owner, &set).unwrap();
    let scope = CleanupScope::of(&ResourceLease::issued(lease, owner, set.clone()));

    let broker = ConfirmedBroker;
    let quiesced =
        futures_executor::block_on(confirm_quiesced_for(&broker, &scope, &held)).unwrap();
    let released = futures_executor::block_on(confirm_released(&broker, &scope, quiesced)).unwrap();
    assert!(scheduler.finish_quiescing(held, released).is_ok());
    assert!(scheduler.holder_of(resource).is_none());
    assert!(store.fences().unwrap().is_empty());
    drop(scheduler);
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert!(reopened.fences().unwrap().is_empty());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
