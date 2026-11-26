#[test]
fn public_socket_requires_private_directory_and_preserves_other_file_modes() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = std::env::temp_dir().join(format!("carosign-client-public-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let protected = dir.join("protected");
    std::fs::write(&protected, b"test").unwrap();
    std::fs::set_permissions(&protected, std::fs::Permissions::from_mode(0o600)).unwrap();
    let socket = dir.join("control.sock");
    let listener = super::publish_client_socket(socket.to_str().unwrap()).unwrap();
    assert_eq!(std::fs::metadata(&socket).unwrap().mode() & 0o777, 0o666);
    assert_eq!(std::fs::metadata(&protected).unwrap().mode() & 0o777, 0o600);
    drop(listener);
    std::fs::remove_file(&socket).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(super::publish_client_socket(socket.to_str().unwrap()).is_err());
    assert!(!socket.exists());
    std::fs::remove_file(protected).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
#[test]
#[ignore = "нужны CAROSIGN_TEST_BROKER и CAROSIGN_TEST_DECLARATION_PROVIDER"]
fn device_declaration_runs_without_device_owner_or_fence() {
    use carosign_persistence::redb::backend::InitialFenceStore;
    let path = std::env::temp_dir().join(format!(
        "carosign-declaration-fence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = InitialFenceStore::create_new(&path).unwrap();
    let resources = crate::resources::LocalResourceOwner::from_initial_fence_store(store).unwrap();
    let profile = crate::provider_supervisor::ProviderProfile::device_fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_DECLARATION_PROVIDER")
            .unwrap()
            .into(),
    );
    let clock = carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    );
    let declared = super::probe_declaration(&profile, &clock).unwrap();
    assert_eq!(
        declared.modality,
        carosign_protocol::carosign::common::v1::Modality::Fingerprint as i32
    );
    assert!(declared.operations.is_empty());
    drop(resources);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert!(reopened.fences().unwrap().is_empty());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn device_capability_probe_never_starts_worker_or_consumes_fence() {
    use carosign_persistence::redb::backend::InitialFenceStore;
    use std::os::unix::fs::PermissionsExt;
    let directory = std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(format!(
        ".carosign-device-probe-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let marker = directory.join("spawned");
    let script = directory.join("worker");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let profile =
        crate::provider_supervisor::ProviderProfile::device_fixture(script.clone(), script);
    let path = directory.join("fences.redb");
    let store = InitialFenceStore::create_new(&path).unwrap();
    let resources = crate::resources::LocalResourceOwner::from_initial_fence_store(store).unwrap();
    let clock = carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    );
    let state = super::device_readiness(Some(Default::default()), || {
        super::probe_capabilities(&profile, &resources, &clock)
    });
    assert!(!state.verified_ready);
    assert!(!marker.exists());
    drop(resources);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert!(reopened.fences().unwrap().is_empty());
    let resource_ids = profile
        .identity()
        .resources
        .claims()
        .iter()
        .map(|claim| claim.id)
        .collect::<Vec<_>>();
    reopened.fence_resources(&resource_ids).unwrap();
    let resources =
        crate::resources::LocalResourceOwner::from_initial_fence_store(reopened).unwrap();
    let error = super::probe_capabilities(&profile, &resources, &clock).unwrap_err();
    assert_eq!(
        error.to_string(),
        "ресурс остаётся ограждённым после восстановления"
    );
    assert!(!marker.exists());
    drop(resources);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(reopened.fences().unwrap(), resource_ids);
    drop(reopened);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn device_declaration_requires_successful_probe() {
    use carosign_protocol::carosign::common::v1::{CapabilityDescriptor, ProviderOperation};
    use carosign_protocol::carosign::common::v1::{
        DeclaredAssurance, PresentationAttackDetection, PresentationAttackDetectionSupport,
    };
    let declared = CapabilityDescriptor {
        assurance: Some(DeclaredAssurance {
            presentation_attack_detection: Some(PresentationAttackDetection {
                support: PresentationAttackDetectionSupport::Declared as i32,
                covered_attacks: Vec::new(),
            }),
        }),
        modality: carosign_protocol::carosign::common::v1::Modality::Fingerprint as i32,
        operations: vec![ProviderOperation::ProviderVerify as i32],
        ..Default::default()
    };
    let failed = super::device_readiness(Some(declared.clone()), || {
        Err(std::io::Error::other("unknown"))
    });
    assert!(!failed.verified_ready);
    assert_eq!(failed.descriptor, Some(declared.clone()));
    let ready = super::device_readiness(Some(declared.clone()), || {
        Ok(CapabilityDescriptor {
            assurance: None,
            ..declared.clone()
        })
    });
    assert!(ready.verified_ready);
    assert_eq!(ready.descriptor, Some(declared));
}

#[test]
#[ignore = "требует собранные CAROSIGN_TEST_BROKER и CAROSIGN_TEST_PROVIDER"]
fn capability_probe_processes_can_be_repeated_after_scoped_cleanup() {
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let resources = crate::resources::LocalResourceOwner::default();
    let clock = carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    );
    for _ in 0..2 {
        let capabilities = super::probe_capabilities(&profile, &resources, &clock).unwrap();
        assert_eq!(capabilities.modality, profile.identity().modality as i32);
    }
}

#[test]
fn configured_fences_are_restored_before_resource_issuance() {
    use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
    use carosign_core::ids::{AuthenticatorId, LeaseId, OperationId, ResourceId, WorkerInstanceId};
    use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
    use carosign_persistence::redb::backend::{FenceScope, InitialFenceStore};
    use carosign_runtime::ports::{ResourceDenial, ResourceOwner, ResourcePort};

    // Root /tmp and /dev /shm do not pass the protected directory check.
    let root = if unsafe { libc::geteuid() } == 0 {
        std::path::PathBuf::from("/root")
    } else {
        std::env::temp_dir()
    };
    let directory = root.join(format!(
        "carosign-fence-startup-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("fences.redb");
    assert!(super::restore_resource_owner(path.to_str()).is_err());
    assert!(!path.exists());
    let store = InitialFenceStore::create_new(&path).unwrap();
    let id = ResourceId::from_bytes([91; 16]).unwrap();
    store.fence(id).unwrap();
    drop(store);
    let restored = super::restore_resource_owner(path.to_str()).unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([92; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([93; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([94; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let set = ResourceSet::new(vec![ResourceClaim {
        id,
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    assert!(!restored.startup_cleanup_confirmed(owner.operation));
    assert_eq!(
        futures_executor::block_on(restored.acquire_for(owner, &set)),
        Err(ResourceDenial::Busy)
    );
    assert!(InitialFenceStore::open_guarded(&path).is_err());
    drop(restored);
    assert!(InitialFenceStore::open_guarded(&path).is_ok());
    let scoped_path = directory.join("scoped.redb");
    let store = InitialFenceStore::create_new(&scoped_path).unwrap();
    store
        .fence_scoped(&FenceScope {
            lease: LeaseId::from_bytes([95; 16]).unwrap(),
            operation: owner.operation,
            worker: owner.worker,
            authenticator: owner.authenticator,
            resources: set,
        })
        .unwrap();
    drop(store);
    let restored = super::restore_resource_owner(scoped_path.to_str()).unwrap();
    assert!(!restored.startup_cleanup_confirmed(owner.operation));
    assert!(restored.startup_cleanup_confirmed(OperationId::from_bytes([96; 16]).unwrap()));
    drop(restored);
    std::fs::remove_file(scoped_path).unwrap();
    let link = directory.join("link.redb");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(InitialFenceStore::create_new(&path).is_err());
    assert!(InitialFenceStore::create_new(&link).is_err());
    assert!(super::restore_resource_owner(link.to_str()).is_err());
    std::fs::remove_file(link).unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
use super::*;
use carosign_protocol::carosign::client::v1::client_frame::Payload;

#[test]
#[ignore = "требует CAROSIGN_TEST_BROKER и CAROSIGN_TEST_PROVIDER"]
fn inventory_recovers_after_initial_provider_failure() {
    let path = std::env::temp_dir().join(format!("carosign-probe-retry-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let socket = path.join("socket");
    let policy_path = path.join("policy");
    std::fs::write(
        &policy_path,
        format!(
            "uid={} subject={} actions=verify,inventory purpose=session-login\n",
            unsafe { libc::getuid() },
            unsafe { libc::getuid() }
        ),
    )
    .unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&path.join("store.redb"))
            .unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
    {
        use crate::health::HealthSource;
        let activity = MutationActivity::default();
        let capabilities = std::sync::Mutex::new(CapabilityState::default());
        let health = LinuxHealth {
            store: &store,
            policy: &policy,
            capabilities: &capabilities,
            activity: &activity,
        };
        let active = activity.enter().unwrap();
        assert!(health.snapshot().is_err());
        drop(active);
        let snapshot = health.snapshot().unwrap();
        assert_eq!(snapshot.scopes_needing_reconciliation, 0);
        assert_eq!(snapshot.unavailable_authenticators, 1);
    }
    let broker_link = path.join("broker");
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        broker_link.clone(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let config = crate::config::Config {
        socket: socket.to_str().unwrap().into(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _ = serve(&config, &store, &policy, &profile, clock);
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    let connection =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let list = || {
        let frame = connection
            .request(Payload::ListAuthenticators(
                carosign_protocol::carosign::client::v1::ListAuthenticators {},
            ))
            .unwrap();
        let Some(Payload::Authenticators(entries)) = frame.payload else {
            panic!("нет инвентаря")
        };
        entries.entries[0].available
    };
    assert!(!list());
    std::os::unix::fs::symlink(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap(),
        broker_link,
    )
    .unwrap();
    assert!(list());
}

#[test]
fn failed_capability_probe_keeps_device_unavailable_and_service_responsive() {
    let path = std::env::temp_dir().join(format!("carosign-linux-service-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let socket = path.join("socket");
    let policy_dir = path.join("policies");
    std::fs::create_dir(&policy_dir).unwrap();
    let policy_path = policy_dir.join("policy");
    std::fs::write(
        &policy_path,
        format!(
            "uid={} subject={} actions=verify,inventory,observe,cancel purpose=session-login\n",
            unsafe { libc::getuid() },
            unsafe { libc::getuid() }
        ),
    )
    .unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&path.join("store.redb"))
            .unwrap();
    let uid = unsafe { libc::getuid() };
    let operation_id = seed_prepared_operation(&store, uid, 36);
    let foreign_operation = seed_prepared_operation(&store, uid ^ 1, 50);
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        "/bin/true".into(),
        "/bin/true".into(),
    );
    {
        use crate::health::HealthSource;
        let activity = MutationActivity::default();
        let capabilities = std::sync::Mutex::new(CapabilityState::default());
        let health = LinuxHealth {
            store: &store,
            policy: &policy,
            capabilities: &capabilities,
            activity: &activity,
        };
        let active = activity.enter().unwrap();
        assert!(health.snapshot().is_err());
        drop(active);
        assert_eq!(health.snapshot().unwrap().scopes_needing_reconciliation, 2);
    }
    let config = crate::config::Config {
        socket: socket.to_str().unwrap().into(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _ = serve(&config, &store, &policy, &profile, clock);
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    // The connection occupied by the inactive client should not interfere with the second.
    let _idle = carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
        .unwrap();
    let transport =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let answer = transport
        .request(Payload::ListAuthenticators(
            carosign_protocol::carosign::client::v1::ListAuthenticators {},
        ))
        .unwrap();
    let Some(Payload::Authenticators(entries)) = answer.payload else {
        panic!("нет инвентаря")
    };
    assert_eq!(entries.entries.len(), 1);
    assert_eq!(entries.entries[0].authenticator_id, vec![5; 16]);
    // `/bin/true` He doesn’t talk to the mediator: the possibility isn’t invented.
    assert!(!entries.entries[0].available);
    let descriptor = entries.entries[0].capabilities.as_ref().unwrap();
    assert_eq!(
        descriptor.modality,
        carosign_protocol::carosign::common::v1::Modality::Fingerprint as i32,
    );
    assert!(descriptor.operations.is_empty());
    use carosign_protocol::carosign::{client::v1::GetAuthenticator, common::v1::ErrorCode};
    let lookup = |authenticator_id| {
        transport
            .request(Payload::GetAuthenticator(GetAuthenticator {
                authenticator_id,
            }))
            .unwrap()
            .payload
    };
    let Some(Payload::Authenticator(answer)) = lookup(vec![5; 16]) else {
        panic!("нет описания устройства");
    };
    assert_eq!(answer.descriptor.as_ref(), entries.entries.first());
    let Some(Payload::ProtocolError(error)) = lookup(vec![6; 16]) else {
        panic!("ожидался отказ");
    };
    assert_eq!(
        error.error.unwrap().code,
        ErrorCode::ErrorUnavailable as i32
    );
    for authenticator_id in [vec![0; 16], vec![5; 15]] {
        assert!(matches!(
            transport.request(Payload::GetAuthenticator(GetAuthenticator {
                authenticator_id
            })),
            Err(carosign_linux_ipc::client::transport::TransportError::Unexpected)
        ));
    }
    use carosign_protocol::carosign::client::v1 as wire;
    let answer = transport
        .request(Payload::GetOperation(wire::GetOperation {
            operation_id: operation_id.as_bytes().to_vec(),
        }))
        .unwrap();
    let Some(Payload::OperationSnapshot(snapshot)) = answer.payload else {
        panic!("нет снимка операции");
    };
    assert_eq!(snapshot.operation_id, operation_id.as_bytes().as_slice());
    assert_eq!(snapshot.phase, wire::OperationPhase::OpTerminal as i32);
    assert_eq!(snapshot.last_event_sequence, 0);
    let Some(wire::terminal_result::Kind::Mutation(result)) =
        snapshot.terminal.clone().unwrap().kind
    else {
        panic!("нет итога операции");
    };
    assert_eq!(result.result, wire::MutationResult::MutationNoChange as i32);
    assert!(result.enrollment.is_none());
    assert!(result.mutation_id.is_empty());

    let completed = transport
        .request(Payload::CancelOperation(wire::CancelOperation {
            operation_id: operation_id.as_bytes().to_vec(),
        }))
        .unwrap();
    assert!(
        matches!(completed.payload, Some(Payload::CancellationReceipt(receipt))
        if receipt.result == wire::CancelReceiptKind::CancelReceiptAlreadyTerminal as i32)
    );
    let forbidden = transport
        .request(Payload::CancelOperation(wire::CancelOperation {
            operation_id: foreign_operation.as_bytes().to_vec(),
        }))
        .unwrap();
    assert!(
        matches!(forbidden.payload, Some(Payload::ProtocolError(error))
        if error.error.as_ref().is_some_and(|error| error.code == ErrorCode::ErrorUnauthorized as i32))
    );

    let observed = transport
        .request(Payload::ObserveOperation(wire::ObserveOperation {
            operation_id: operation_id.as_bytes().to_vec(),
            after_sequence: 0,
        }))
        .unwrap();
    assert_eq!(observed.payload, Some(Payload::OperationSnapshot(snapshot)));

    for operation_id in [foreign_operation.as_bytes().to_vec(), vec![98; 16]] {
        let answer = transport
            .request(Payload::GetOperation(wire::GetOperation { operation_id }))
            .unwrap();
        let Some(Payload::ProtocolError(error)) = answer.payload else {
            panic!("ожидался отказ");
        };
        assert_eq!(
            error.error.unwrap().code,
            ErrorCode::ErrorUnauthorized as i32
        );
    }
    let answer = transport
        .request(Payload::ObserveOperation(wire::ObserveOperation {
            operation_id: foreign_operation.as_bytes().to_vec(),
            after_sequence: 0,
        }))
        .unwrap();
    let Some(Payload::ProtocolError(error)) = answer.payload else {
        panic!("ожидался отказ подписки");
    };
    assert_eq!(
        error.error.unwrap().code,
        ErrorCode::ErrorUnauthorized as i32
    );

    let answer = transport
        .request(Payload::GetHealth(
            carosign_protocol::carosign::client::v1::GetHealth {},
        ))
        .unwrap();
    let Some(Payload::Health(health)) = answer.payload else {
        panic!("нет состояния службы")
    };
    assert!(health.service_ready);
    assert_eq!(health.available_authenticators, 0);
    assert_eq!(health.unavailable_authenticators, 1);
    assert_eq!(health.scopes_needing_reconciliation, 0);
}

#[test]
#[ignore = "требует CAROSIGN_TEST_BROKER и CAROSIGN_TEST_PROVIDER"]
fn live_service_mock_verification_reaches_client() {
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{AuthorityId, EnrollmentId};
    use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    use carosign_persistence::store::{MetadataStore, Table};
    let path = std::env::temp_dir().join(format!("carosign-linux-e2e-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let socket = path.join("socket");
    let policy_dir = path.join("policies");
    std::fs::create_dir(&policy_dir).unwrap();
    let policy_path = policy_dir.join("policy");
    let uid = unsafe { libc::getuid() };
    std::fs::write(
        &policy_path,
        format!("uid={uid} subject={uid} actions=verify purpose=session-login\n"),
    )
    .unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&path.join("store.redb"))
            .unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes(*b"linux-local-v001").unwrap(),
        subject: carosign_adapters::policy::subject_of(u64::from(uid)),
    };
    let authenticator = carosign_core::ids::AuthenticatorId::from_bytes([5; 16]).unwrap();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([15; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment,
        display_label: None,
        label: "тест".into(),
        status: EnrollmentStatus::Active,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![21; 16]).unwrap(),
        format_id: String::new(),
        format_version: 0,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        enrollment.id.as_bytes(),
        &carosign_persistence::mutation_store::encode_enrollment_v4(&record).unwrap(),
    )
    .unwrap();
    let mut key = crate::accept::scope_of(subject, authenticator).to_vec();
    key.extend_from_slice(enrollment.id.as_bytes());
    tx.put(Table::EnrollmentIndex, &key, enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
    let caller = carosign_runtime::ports::CallerContext::from_transport(
        [1; 32],
        uid,
        carosign_runtime::ports::TransactionId::from_bytes([2; 16]).unwrap(),
    );
    let intent = carosign_runtime::authority::verification::VerificationIntent {
        report_matched_enrollment: false,
        enrollment: None,
        subject: carosign_runtime::authority::verification::RequestedSubject::Current,
        purpose_hint: carosign_runtime::authority::verification::PurposeHint::SessionLogin,
        authority_request_ref: vec![],
        authenticator,
    };
    assert!(
        futures_executor::block_on(crate::accept::prepare_client_verification(
            &store,
            &policy,
            &policy,
            &caller,
            &intent,
            carosign_core::device::DeviceEpoch::FIRST
        ))
        .is_ok()
    );
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let config = crate::config::Config {
        socket: socket.to_str().unwrap().into(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _ = serve(&config, &store, &policy, &profile, clock);
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    let listing =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let selection = carosign_protocol::carosign::common::v1::SubjectSelector {
        selector: Some(
            carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                true,
            ),
        ),
    };
    let list = listing
        .request(Payload::ListEnrollments(
            carosign_protocol::carosign::client::v1::ListEnrollments {
                subject: Some(selection.clone()),
                authenticator_id: authenticator.as_bytes().to_vec(),
                authority_request_ref: vec![],
            },
        ))
        .unwrap();
    let Some(Payload::Enrollments(entries)) = list.payload else {
        panic!("список регистраций недоступен")
    };
    assert_eq!(entries.entries.len(), 1);
    assert_eq!(
        entries.entries[0]
            .enrollment
            .as_ref()
            .unwrap()
            .enrollment_id,
        enrollment.id.as_bytes()
    );
    let denied = listing
        .request(Payload::ListEnrollments(
            carosign_protocol::carosign::client::v1::ListEnrollments {
                subject: Some(selection),
                authenticator_id: authenticator.as_bytes().to_vec(),
                authority_request_ref: vec![42],
            },
        ))
        .unwrap();
    assert!(matches!(denied.payload, Some(Payload::ProtocolError(_))));
    let unknown = listing.request(Payload::ListEnrollments(carosign_protocol::carosign::client::v1::ListEnrollments {
            subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
                selector: Some(carosign_protocol::carosign::common::v1::subject_selector::Selector::AccountHint(
                    "carosign-test-unknown-account-do-not-create".into(),
                )),
            }),
            authenticator_id: authenticator.as_bytes().to_vec(), authority_request_ref: vec![],
        })).unwrap();
    assert!(matches!(unknown.payload, Some(Payload::ProtocolError(_))));
    drop(listing);
    let request = carosign_protocol::carosign::client::v1::BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: vec![1; 16],
        subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
            selector: Some(
                carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                    true,
                ),
            ),
        }),
        authenticator_id: authenticator.as_bytes().to_vec(),
        purpose_hint:
            carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionLogin as i32,
        authority_request_ref: vec![],
    };
    let expected = match std::env::var("CAROSIGN_TEST_SCENARIO").as_deref() {
        Ok("linux-matched-scope") => pam_carosign::client::Observation::FactorAccepted,
        _ => pam_carosign::client::Observation::NoSample,
    };
    let interrupted =
        carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
            .unwrap();
    let accepted = interrupted
        .request(Payload::BeginVerification(
            carosign_protocol::carosign::client::v1::BeginVerification {
                report_matched_enrollment: false,
                enrollment: None,
                idempotency_key: vec![2; 16],
                ..request.clone()
            },
        ))
        .unwrap();
    assert!(matches!(
        accepted.payload,
        Some(Payload::OperationAccepted(_))
    ));
    drop(interrupted);
    for _ in 0..2 {
        let transport =
            carosign_linux_ipc::client::transport::Transport::connect(socket.to_str().unwrap())
                .unwrap();
        let result = pam_carosign::client::observe_verification(&transport, request.clone());
        assert!(
            matches!(&result, Ok(actual) if std::mem::discriminant(actual) == std::mem::discriminant(&expected)),
            "{result:?}"
        );
    }
}

fn seed_prepared_operation(
    store: &dyn carosign_persistence::store::MetadataStore,
    uid: u32,
    seed: u8,
) -> carosign_core::ids::OperationId {
    use carosign_core::{
        identity::SubjectRef,
        ids::{
            AuthenticatorId, AuthorityId, EnrollmentId, IdempotencyKey, MutationId, OperationId,
        },
    };
    use carosign_persistence::{
        journal::PrepareMutation,
        model::IdempotencyRecord,
        mutation_plan::{MutationPlan, Scope, Transition},
    };
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes(*b"linux-local-v001").unwrap(),
        subject: carosign_adapters::policy::subject_of(u64::from(uid)),
    };
    let authenticator = AuthenticatorId::from_bytes([5; 16]).unwrap();
    let operation = OperationId::from_bytes([seed; 16]).unwrap();
    let mutation = MutationId::from_bytes([seed + 1; 16]).unwrap();
    let prepared = PrepareMutation {
        mutation,
        operation,
        subject,
        authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([seed + 2; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject,
                authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [seed + 3; 32],
        key: IdempotencyKey::from_bytes([seed + 4; 16]).unwrap(),
        request_digest: [seed + 5; 32],
        operation,
        mutation: Some(mutation),
    };
    let mut tx = store.begin_write().unwrap();
    carosign_persistence::mutation_store::stage_prepared_v4(&mut *tx, &prepared, &request).unwrap();
    tx.commit_durable().unwrap();
    operation
}
