use super::*;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::ResourceId;
use carosign_core::resource::{AccessMode, ResourceClaim};

#[test]
fn failed_authorization_revoke_cannot_prove_cleanup() {
    for panic_revoke in [false, true] {
        let worker = carosign_core::ids::WorkerInstanceId::from_bytes([2; 16]).unwrap();
        let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let supervisor = crate::provider_supervisor::ProviderSupervisor::test_exclusive_owner(
            worker,
            pair.platform,
        );
        let cleanup = SupervisorCleanup::new(
            supervisor,
            std::sync::Arc::new(LocalResourceOwner::default()),
        );
        cleanup
            .install_preparation_revoker(std::sync::Arc::new(move || {
                if panic_revoke {
                    panic!("revoker");
                }
                false
            }))
            .unwrap();
        assert!(cleanup.interrupt().is_err());
        assert!(cleanup.stopped_scope.lock().unwrap().is_none());
    }
}

#[test]
fn cancellation_racing_preparation_registration_never_leaves_installed_revoker_unfired() {
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    for _ in 0..8 {
        let worker = carosign_core::ids::WorkerInstanceId::from_bytes([2; 16]).unwrap();
        let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let supervisor = crate::provider_supervisor::ProviderSupervisor::test_exclusive_owner(
            worker,
            pair.platform,
        );
        let cleanup = Arc::new(SupervisorCleanup::new(
            supervisor,
            Arc::new(LocalResourceOwner::default()),
        ));
        let count = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(2));
        let thread = {
            let cleanup = cleanup.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                cleanup.interrupt()
            })
        };
        let observed = count.clone();
        barrier.wait();
        let installed = cleanup
            .install_preparation_revoker(Arc::new(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                true
            }))
            .is_ok();
        let _ = thread.join().unwrap();
        if installed {
            assert_eq!(count.load(Ordering::SeqCst), 1);
        }
        assert!(
            cleanup
                .install_preparation_revoker(Arc::new(|| true))
                .is_err()
        );
    }
}

fn set() -> ResourceSet {
    ResourceSet::new(vec![ResourceClaim {
        id: ResourceId::from_bytes([1u8; 16]).expect("валид"),
        mode: AccessMode::Exclusive,
    }])
    .expect("набор")
}

#[test]
fn only_quiesced_exclusive_lease_releases_durable_fence() {
    use carosign_core::{
        device::AuthenticatorRef,
        ids::{AuthenticatorId, OperationId, WorkerInstanceId},
    };
    use carosign_persistence::redb::backend::InitialFenceStore;
    use carosign_protocol::carosign::broker_relay::v1::{
        Control, ControlResult, control, control_result,
    };
    use carosign_runtime::scheduler::cleanup::{confirm_quiesced_for, confirm_released};
    use prost::Message;
    use std::{os::fd::AsRawFd, sync::Arc};

    for scenario in [
        "quiesced",
        "interrupted",
        "authorization_failed",
        "race",
        "exclusive",
        "ordinary",
        "lost",
    ] {
        let path = std::env::temp_dir().join(format!(
            "carosign-exclusive-{}-{scenario}",
            std::process::id()
        ));
        let store = InitialFenceStore::create_new(&path).unwrap();
        let resources = Arc::new(LocalResourceOwner::from_initial_fence_store(store).unwrap());
        let owner = ResourceOwner {
            operation: OperationId::from_bytes([3; 16]).unwrap(),
            worker: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
                device_epoch: DeviceEpoch::FIRST,
            },
        };
        let lease = futures_executor::block_on(resources.acquire_for(owner, &set())).unwrap();
        let scope = CleanupScope::of(&lease);
        resources
            .with_serialized_binding(&scope, true, || ())
            .unwrap();
        let held = resources
            .scheduler
            .lock()
            .unwrap()
            .begin_quiescing(scope.lease_id(), owner, scope.resource_set())
            .unwrap();
        let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let supervisor = crate::provider_supervisor::ProviderSupervisor::test_exclusive_owner(
            owner.worker,
            pair.platform,
        );
        assert!(supervisor.requires_durable_binding());
        assert!(supervisor.resource_slots_for_binding().unwrap().is_empty());
        assert!(futures_executor::block_on(resources.acquire_for(owner, &set())).is_err());
        let cleanup = Arc::new(SupervisorCleanup::new_for_scope(
            supervisor,
            resources.clone(),
            scope.clone(),
        ));
        let expected = relay_scope(&scope);
        let server = std::thread::spawn(move || {
            let mut bytes = [0u8; 1024];
            let n = unsafe {
                libc::recv(
                    pair.worker.as_raw_fd(),
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    0,
                )
            };
            let request = Control::decode(&bytes[..n as usize]).unwrap();
            let Some(control::Action::Revoke(revoke)) = request.action else {
                panic!("ожидался отзыв")
            };
            assert_eq!(revoke.scope, Some(expected));
            if scenario == "lost" {
                return;
            }
            let response = ControlResult {
                request_id: request.request_id,
                result: Some(
                    if matches!(
                        scenario,
                        "quiesced" | "interrupted" | "race" | "authorization_failed"
                    ) {
                        control_result::Result::ExclusiveQuiesced(true)
                    } else if scenario == "exclusive" {
                        control_result::Result::ExclusiveRevoked(true)
                    } else {
                        control_result::Result::Revoked(true)
                    },
                ),
            }
            .encode_to_vec();
            assert_eq!(
                unsafe {
                    libc::send(
                        pair.worker.as_raw_fd(),
                        response.as_ptr().cast(),
                        response.len(),
                        0,
                    )
                },
                response.len() as isize
            );
        });
        if scenario == "interrupted" {
            cleanup.interrupt().unwrap();
        }
        if scenario == "authorization_failed" {
            cleanup
                .install_preparation_revoker(Arc::new(|| false))
                .unwrap();
            assert!(cleanup.interrupt().is_err());
        }
        let racing = if scenario == "race" {
            let cleanup = cleanup.clone();
            Some(std::thread::spawn(move || cleanup.interrupt()))
        } else {
            None
        };
        let proof =
            futures_executor::block_on(confirm_quiesced_for(cleanup.as_ref(), &scope, &held));
        if let Some(racing) = racing {
            racing.join().unwrap().unwrap();
        }
        if matches!(scenario, "quiesced" | "interrupted" | "race") {
            let released = futures_executor::block_on(confirm_released(
                cleanup.as_ref(),
                &scope,
                proof.unwrap(),
            ))
            .unwrap();
            assert!(
                resources
                    .scheduler
                    .lock()
                    .unwrap()
                    .finish_quiescing(held, released)
                    .is_ok()
            );
            assert!(futures_executor::block_on(resources.acquire_for(owner, &set())).is_ok());
        } else {
            assert!(proof.is_err());
            assert!(
                resources
                    .scheduler
                    .lock()
                    .unwrap()
                    .quiescing_uncertain(held)
            );
            assert!(futures_executor::block_on(resources.acquire_for(owner, &set())).is_err());
        }
        server.join().unwrap();
        drop(cleanup);
        drop(resources);
        if scenario == "lost" {
            let store = InitialFenceStore::open_guarded(&path).unwrap();
            let restored = LocalResourceOwner::from_initial_fence_store(store).unwrap();
            assert!(matches!(restored.has_recovered_fence(&set()), Ok(true)));
            assert!(futures_executor::block_on(restored.acquire_for(owner, &set())).is_err());
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn durable_binding_is_serialized_with_worker_revocation() {
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::ids::{AuthenticatorId, OperationId, WorkerInstanceId};
    use carosign_persistence::redb::backend::InitialFenceStore;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    let path = std::env::temp_dir().join(format!(
        "carosign-serialized-bind-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let store = InitialFenceStore::create_new(&path).unwrap();
    let resources = Arc::new(LocalResourceOwner::from_initial_fence_store(store).unwrap());
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = futures_executor::block_on(resources.acquire_for(owner, &set())).unwrap();
    let scope = CleanupScope::of(&lease);
    let wire = relay_scope(&scope);
    assert_eq!(wire.lease_id, scope.lease_id().as_bytes());
    let wire_owner = wire.owner.unwrap();
    assert_eq!(wire_owner.operation_id, owner.operation.as_bytes());
    assert_eq!(wire_owner.worker_instance, owner.worker.as_bytes());
    assert_eq!(
        wire_owner.authenticator_id,
        owner.authenticator.id.as_bytes()
    );
    assert_eq!(
        wire_owner.device_epoch,
        owner.authenticator.device_epoch.value()
    );
    assert_eq!(wire.claims.len(), 1);
    assert_eq!(wire.claims[0].resource_id, [1; 16]);
    assert_eq!(
        wire.claims[0].mode,
        carosign_protocol::carosign::broker_relay::v1::AccessMode::Exclusive as i32
    );
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let sending_resources = Arc::clone(&resources);
    let sending_scope = scope.clone();
    let sending = std::thread::spawn(move || {
        sending_resources.with_serialized_binding(&sending_scope, true, || {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (revoked_tx, revoked_rx) = mpsc::channel();
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let revoking_resources = Arc::clone(&resources);
    let revoking = std::thread::spawn(move || {
        attempt_tx.send(()).unwrap();
        revoking_resources
            .scheduler()
            .lock()
            .unwrap()
            .fence_stopped_worker(owner.worker, owner.authenticator.device_epoch);
        revoked_tx.send(()).unwrap();
    });
    attempt_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(revoked_rx.recv_timeout(Duration::from_millis(50)).is_err());
    release_tx.send(()).unwrap();
    sending.join().unwrap().unwrap();
    revoked_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    revoking.join().unwrap();
    assert_eq!(
        resources.with_serialized_binding(&scope, true, || panic!("отозванная выдача отправлена")),
        Err(ResourceDenial::Denied)
    );
    drop(resources);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(reopened.fences().unwrap(), vec![set().claims()[0].id]);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn device_binding_without_durable_store_never_calls_sender() {
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::ids::{AuthenticatorId, OperationId, WorkerInstanceId};
    let resources = LocalResourceOwner::default();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = futures_executor::block_on(resources.acquire_for(owner, &set())).unwrap();
    let scope = CleanupScope::of(&lease);
    assert_eq!(
        resources.with_serialized_binding(&scope, true, || panic!("Bind без durable fence")),
        Err(ResourceDenial::Unavailable)
    );
}

#[test]
fn recovered_scheduler_holds_store_lock_after_owner_is_dropped() {
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::ids::{AuthenticatorId, OperationId, WorkerInstanceId};
    use carosign_persistence::redb::backend::InitialFenceStore;
    let directory = std::env::temp_dir().join(format!(
        "carosign-fence-owner-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("fences.redb");
    let store = InitialFenceStore::create_new(&path).unwrap();
    store
        .fence(ResourceId::from_bytes([1; 16]).unwrap())
        .unwrap();
    let owner = LocalResourceOwner::from_initial_fence_store(store).unwrap();
    let requester = ResourceOwner {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    assert_eq!(
        futures_executor::block_on(owner.acquire_for(requester, &set())),
        Err(ResourceDenial::Busy)
    );
    let scheduler = owner.scheduler();
    drop(owner);
    assert!(InitialFenceStore::open_guarded(&path).is_err());
    drop(scheduler);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(
        reopened.fences().unwrap(),
        vec![ResourceId::from_bytes([1; 16]).unwrap()]
    );
    drop(reopened);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn local_lease_remains_busy_until_both_processes_exit() {
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::ids::{AuthenticatorId, OperationId, WorkerInstanceId};
    let local = LocalResourceOwner::default();
    let worker_id = WorkerInstanceId::from_bytes([2; 16]).unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        worker: worker_id,
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let resources = set();
    let lease = futures_executor::block_on(local.acquire_for(owner, &resources)).unwrap();
    assert!(local.validate_live(&lease));
    assert!(local.scheduler().lock().unwrap().holds_live_scope(
        lease.lease_id(),
        owner,
        &resources
    ));
    let lease_id = lease.lease_id();
    assert_eq!(
        futures_executor::block_on(local.acquire_for(owner, &resources)),
        Err(ResourceDenial::Busy)
    );
    assert_eq!(
        futures_executor::block_on(local.release(lease)),
        QuiesceAssessment::Uncertain
    );
    let mut worker = std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .unwrap();
    let mut broker = std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .unwrap();
    worker.kill().unwrap();
    worker.wait().unwrap();
    assert_eq!(
        local.confirm_stopped(
            worker_id,
            &mut worker,
            &mut broker,
            ExitCleanup::ProcessOnly
        ),
        Err(ResourceDenial::Busy)
    );
    assert_eq!(
        futures_executor::block_on(local.acquire_for(owner, &resources)),
        Err(ResourceDenial::Busy)
    );
    broker.kill().unwrap();
    broker.wait().unwrap();
    local
        .confirm_stopped(
            worker_id,
            &mut worker,
            &mut broker,
            ExitCleanup::HardwareUnconfirmed,
        )
        .unwrap();
    let lease = ResourceLease::issued(lease_id, owner, resources.clone());
    assert_eq!(
        futures_executor::block_on(local.release(lease)),
        QuiesceAssessment::Uncertain
    );
    assert_eq!(
        futures_executor::block_on(local.acquire_for(owner, &resources)),
        Err(ResourceDenial::Busy)
    );
    local
        .confirm_stopped(
            worker_id,
            &mut worker,
            &mut broker,
            ExitCleanup::ProcessOnly,
        )
        .unwrap();
    let lease = ResourceLease::issued(lease_id, owner, resources.clone());
    assert!(!local.validate_live(&lease));
    assert_eq!(
        futures_executor::block_on(local.release(lease)),
        QuiesceAssessment::Quiesced
    );
    assert_eq!(
        futures_executor::block_on(local.acquire_for(owner, &resources)),
        Err(ResourceDenial::Unavailable)
    );
    let next_owner = ResourceOwner {
        worker: WorkerInstanceId::from_bytes([5; 16]).unwrap(),
        ..owner
    };
    assert!(futures_executor::block_on(local.acquire_for(next_owner, &resources)).is_ok());
}

fn lease() -> LeaseId {
    LeaseId::from_bytes([2u8; 16]).expect("валид")
}

#[test]
fn no_broker_refuses_with_its_own_word() {
    let result = futures_executor::block_on(NoBroker.acquire(&set(), lease()));
    assert_eq!(result, Err(ResourceDenial::Unavailable));
    let owner = carosign_runtime::ports::ResourceOwner {
        operation: carosign_core::ids::OperationId::from_bytes([3; 16]).unwrap(),
        worker: carosign_core::ids::WorkerInstanceId::from_bytes([4; 16]).unwrap(),
        authenticator: carosign_core::device::AuthenticatorRef {
            id: carosign_core::ids::AuthenticatorId::from_bytes([5; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    assert_eq!(
        futures_executor::block_on(NoBroker.acquire_for(owner, &set())),
        Err(ResourceDenial::Unavailable),
    );
    // ⚠️ "Unknown"  ⁇  "comforted."
    let release = futures_executor::block_on(ResourcePort::release(
        &NoBroker,
        ResourceLease::issued_for_fixture(
            lease(),
            carosign_runtime::ports::ResourceOwner {
                operation: carosign_core::ids::OperationId::from_bytes([3; 16]).unwrap(),
                worker: carosign_core::ids::WorkerInstanceId::from_bytes([4; 16]).unwrap(),
                authenticator: carosign_core::device::AuthenticatorRef {
                    id: carosign_core::ids::AuthenticatorId::from_bytes([5; 16]).unwrap(),
                    device_epoch: DeviceEpoch::FIRST,
                },
            },
            set(),
        ),
    ));
    assert_eq!(release, QuiesceAssessment::Uncertain);
}
