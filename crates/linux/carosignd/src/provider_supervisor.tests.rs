#[test]
fn extra_resource_requires_an_id_role_and_absolute_clean_path() {
    let resource =
        super::parse_extra_resource("08080808080808080808080808080808:config:/etc/carosign/config")
            .unwrap();
    assert_eq!(resource.id, [8; 16]);
    assert_eq!(resource.path, std::path::Path::new("/etc/carosign/config"));
    assert!(
        super::parse_extra_resource("00000000000000000000000000000000:device:/dev/test").is_err()
    );
    assert!(
        super::parse_extra_resource("08080808080808080808080808080808:unknown:/dev/test").is_err()
    );
    assert!(
        super::parse_extra_resource("08080808080808080808080808080808:config:relative").is_err()
    );
    assert!(
        super::parse_extra_resource("08080808080808080808080808080808:config:/etc/../tmp/test")
            .is_err()
    );
}
use super::*;

#[test]
fn challenged_owner_requires_all_profile_boundaries() {
    let mut profile = ProviderProfile::fixture("/bin/true".into(), "/bin/true".into());
    assert!(!profile.challenged_owner_configured());
    profile.resource_owner = Some("/bin/true".into());
    profile.exclusive_resource_owner = true;
    profile.enrollment_owner_endpoint = true;
    profile.enrollment_recipient = Some(
        carosign_runtime::enrollment_authorization::EnrollmentRecipientId::from_bytes([9; 16])
            .unwrap(),
    );
    assert!(!profile.challenged_owner_configured());
    profile.owner_worker_endpoint = true;
    assert!(profile.challenged_owner_configured());
    profile.enrollment_owner_endpoint = false;
    assert!(!profile.challenged_owner_configured());
}

#[test]
fn owner_gets_only_device_resources_in_original_order() {
    use carosign_protocol::carosign::provider::v1::FdRole;
    let mut profile = ProviderProfile::device_fixture("/bin/true".into(), "/bin/true".into());
    profile.additional_resources = vec![
        SystemResource {
            path: "/model".into(),
            role: FdRole::Config,
            id: [8; 16],
        },
        SystemResource {
            path: "/device".into(),
            role: FdRole::Device,
            id: [9; 16],
        },
    ];
    assert_eq!(
        profile.owner_resources().collect::<Vec<_>>(),
        vec![[7; 16], [9; 16]]
    );
}

#[test]
fn executable_must_be_root_owned_and_not_writable_by_others() {
    assert!(protected_executable(Path::new("/usr/bin/true")).is_ok());
    let path = std::env::temp_dir().join(format!("carosign-exec-{}", std::process::id()));
    std::fs::write(&path, b"not an executable").unwrap();
    assert!(protected_executable(&path).is_err());
    std::fs::remove_file(path).unwrap();
    assert!(protected_executable(Path::new("relative")).is_err());
}

#[test]
fn enrollment_endpoint_requires_owner_and_binding_and_is_single_use() {
    let (endpoint, _peer) = pair().unwrap();
    let mut supervisor = ProviderSupervisor {
        modality: carosign_protocol::carosign::common::v1::Modality::Fingerprint,
        worker: SupervisedWorker::Child(Command::new("sleep").arg("60").spawn().unwrap()),
        broker: Command::new("sleep").arg("60").spawn().unwrap(),
        resource_owner: Some(Command::new("sleep").arg("60").spawn().unwrap()),
        exclusive_resource_owner: false,
        worker_id: carosign_core::ids::WorkerInstanceId::from_bytes([2; 16]).unwrap(),
        tunnel: None,
        control: None,
        enrollment_endpoint: Some(endpoint),
        enrollment_control_endpoint: None,
        resource_fd: None,
        additional_resources: Vec::new(),
        resource_binding_started: false,
        durable_binding: false,
    };
    assert!(supervisor.take_enrollment_endpoint().is_none());
    supervisor.mark_resource_binding_started();
    assert!(supervisor.take_enrollment_endpoint().is_none());
    supervisor.exclusive_resource_owner = true;
    assert!(supervisor.take_enrollment_endpoint().is_some());
    assert!(supervisor.take_enrollment_endpoint().is_none());
}

#[test]
fn hardware_binding_is_closed_without_durable_owner() {
    use carosign_protocol::carosign::provider::v1::FdRole;
    let mut supervisor = ProviderSupervisor {
        modality: carosign_protocol::carosign::common::v1::Modality::Fingerprint,
        worker: SupervisedWorker::Child(Command::new("sleep").arg("60").spawn().unwrap()),
        broker: Command::new("sleep").arg("60").spawn().unwrap(),
        resource_owner: None,
        exclusive_resource_owner: false,
        worker_id: carosign_core::ids::WorkerInstanceId::from_bytes([2; 16]).unwrap(),
        tunnel: None,
        control: None,
        enrollment_endpoint: None,
        enrollment_control_endpoint: None,
        resource_fd: None,
        additional_resources: Vec::new(),
        resource_binding_started: false,
        durable_binding: false,
    };
    assert!(supervisor.resource_slots_for_binding().unwrap().is_empty());
    supervisor.resource_fd = Some(SystemResource {
        path: "/dev/null".into(),
        role: FdRole::Config,
        id: [7; 16],
    });
    assert_eq!(
        supervisor.resource_slots_for_binding().unwrap(),
        vec![([7; 16], FdRole::Config as i32)]
    );
    supervisor.additional_resources.push(SystemResource {
        path: "/dev/null".into(),
        role: FdRole::Config,
        id: [8; 16],
    });
    assert_eq!(supervisor.resource_slots_for_binding().unwrap().len(), 2);
    supervisor.additional_resources[0].role = FdRole::Device;
    assert!(supervisor.resource_slots_for_binding().is_err());
    supervisor.additional_resources[0].role = FdRole::Config;
    supervisor.resource_fd.as_mut().unwrap().role = FdRole::Device;
    assert!(supervisor.resource_slots_for_binding().is_err());
    assert!(!supervisor.resource_binding_started);
}

#[test]
fn stopped_processes_do_not_prove_device_quiescence_after_binding() {
    use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
    use carosign_core::ids::{AuthenticatorId, OperationId, ResourceId, WorkerInstanceId};
    use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
    use carosign_protocol::carosign::provider::v1::FdRole;
    use carosign_runtime::ports::{ResourceOwner, ResourcePort};

    let worker_id = WorkerInstanceId::from_bytes([2; 16]).unwrap();
    let owner_process = Command::new("sleep").arg("60").spawn().unwrap();
    let owner_pid = owner_process.id();
    let supervisor = ProviderSupervisor {
        modality: carosign_protocol::carosign::common::v1::Modality::Fingerprint,
        worker: SupervisedWorker::Child(Command::new("sleep").arg("60").spawn().unwrap()),
        broker: Command::new("sleep").arg("60").spawn().unwrap(),
        resource_owner: Some(owner_process),
        exclusive_resource_owner: false,
        worker_id,
        tunnel: None,
        control: None,
        enrollment_endpoint: None,
        enrollment_control_endpoint: None,
        resource_fd: Some(SystemResource {
            path: "/dev/null".into(),
            role: FdRole::Device,
            id: [7; 16],
        }),
        additional_resources: Vec::new(),
        resource_binding_started: true,
        durable_binding: true,
    };
    let path = std::env::temp_dir().join(format!(
        "carosign-owner-fence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = carosign_persistence::redb::backend::InitialFenceStore::create_new(&path).unwrap();
    let resources = std::sync::Arc::new(
        crate::resources::LocalResourceOwner::from_initial_fence_store(store).unwrap(),
    );
    let set = ResourceSet::new(vec![ResourceClaim {
        id: ResourceId::from_bytes([7; 16]).unwrap(),
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([8; 16]).unwrap(),
        worker: worker_id,
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([9; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = futures_executor::block_on(resources.acquire_for(owner, &set)).unwrap();
    let scope = crate::resources::CleanupScope::of(&lease);
    resources
        .with_serialized_binding(&scope, true, || ())
        .unwrap();
    let scheduler = resources.scheduler();
    let held = scheduler
        .lock()
        .unwrap()
        .begin_quiescing(lease.lease_id(), owner, &set)
        .unwrap();
    let cleanup = crate::resources::SupervisorCleanup::new(supervisor, resources);
    assert!(
        futures_executor::block_on(carosign_runtime::scheduler::cleanup::confirm_quiesced_for(
            &cleanup, &scope, &held
        ),)
        .is_err()
    );
    assert!(
        scheduler
            .lock()
            .unwrap()
            .holder_of(set.claims()[0].id)
            .is_some()
    );
    // SAFETY: Kill(pid, 0) merely checks for the existence of an expected PID.
    assert_eq!(unsafe { libc::kill(owner_pid as i32, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    drop(cleanup);
    drop(scheduler);
    let reopened =
        carosign_persistence::redb::backend::InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(reopened.fences().unwrap(), vec![set.claims()[0].id]);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn creates_private_seqpacket_connections() {
    let (left, right) = pair().unwrap();
    let mut kind = 0;
    let mut size = std::mem::size_of_val(&kind) as libc::socklen_t;
    assert_eq!(
        unsafe {
            libc::getsockopt(
                left.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut i32).cast(),
                &mut size,
            )
        },
        0
    );
    assert_eq!(kind, libc::SOCK_SEQPACKET);
    assert_ne!(left.as_raw_fd(), right.as_raw_fd());
    assert_ne!(
        unsafe { libc::fcntl(right.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
}

#[test]
#[ignore = "требует заранее собранные пути CAROSIGN_TEST_BROKER и CAROSIGN_TEST_PROVIDER"]
fn starts_real_broker_and_mock_worker() {
    let profile = ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let boot = carosign_linux::sys::boot_id().unwrap();
    let mut supervisor = ProviderSupervisor::start(
        &profile,
        boot,
        carosign_core::device::DeviceEpoch::FIRST,
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(!supervisor.worker.has_exited().unwrap());
    assert!(supervisor.broker.try_wait().unwrap().is_none());
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::ids::{AuthenticatorId, OperationId, ResourceId};
    use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
    use carosign_runtime::ports::{ResourceOwner, ResourcePort};
    let resources = std::sync::Arc::new(crate::resources::LocalResourceOwner::default());
    let set = ResourceSet::new(vec![ResourceClaim {
        id: ResourceId::from_bytes([7; 16]).unwrap(),
        mode: AccessMode::Exclusive,
    }])
    .unwrap();
    let owner = ResourceOwner {
        operation: OperationId::from_bytes([8; 16]).unwrap(),
        worker: supervisor.worker_id(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([9; 16]).unwrap(),
            device_epoch: carosign_core::device::DeviceEpoch::FIRST,
        },
    };
    let lease = futures_executor::block_on(resources.acquire_for(owner, &set)).unwrap();
    let scope = crate::resources::CleanupScope::of(&lease);
    let scheduler = resources.scheduler();
    let held = scheduler
        .lock()
        .unwrap()
        .begin_quiescing(lease.lease_id(), owner, &set)
        .unwrap();
    let cleanup = crate::resources::SupervisorCleanup::new(supervisor, resources);
    let quiesced = futures_executor::block_on(
        carosign_runtime::scheduler::cleanup::confirm_quiesced_for(&cleanup, &scope, &held),
    )
    .unwrap();
    let released = futures_executor::block_on(
        carosign_runtime::scheduler::cleanup::confirm_released(&cleanup, &scope, quiesced),
    )
    .unwrap();
    assert!(
        scheduler
            .lock()
            .unwrap()
            .finish_quiescing(held, released)
            .is_ok()
    );
    assert!(
        scheduler
            .lock()
            .unwrap()
            .holder_of(set.claims()[0].id)
            .is_none()
    );
}

#[test]
fn worker_exec_has_no_capabilities_and_cannot_gain_privileges() {
    let mut command = Command::new("/bin/cat");
    command.arg("/proc/self/status");
    restrict_worker(&mut command);
    let output = command.output().unwrap();
    assert!(output.status.success());
    let status = String::from_utf8(output.stdout).unwrap();
    for field in ["CapInh:", "CapPrm:", "CapEff:", "CapAmb:"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(field))
            .unwrap()
            .trim();
        assert_eq!(u64::from_str_radix(value, 16).unwrap(), 0, "{field}");
    }
    assert!(
        status
            .lines()
            .any(|line| line.split_whitespace().eq(["NoNewPrivs:", "1"]))
    );
}

/// External test of two issues; resource preparation checks the launcher provider.
#[test]
#[ignore = "нужны изолированные ресурсы и проверенные временные профили"]
fn external_device_two_leases() {
    use carosign_core::{
        device::{AuthenticatorRef, DeviceEpoch},
        ids::{OperationId, ResourceId},
        resource::{AccessMode, ResourceClaim, ResourceSet},
    };
    use carosign_persistence::redb::backend::InitialFenceStore;
    use carosign_runtime::{
        ports::{ResourceOwner, ResourcePort},
        scheduler::cleanup::{confirm_quiesced_for, confirm_released},
    };
    use std::{path::PathBuf, sync::Arc};
    let root = PathBuf::from(std::env::var_os("CAROSIGN_TEST_ROOT").expect("CAROSIGN_TEST_ROOT"));
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with("/tmp") && root != PathBuf::from("/tmp"));
    let path = root.join("fences.redb");
    let mut store = Arc::new(InitialFenceStore::create_new(&path).unwrap());
    let boot = carosign_linux::sys::boot_id().unwrap();
    let set = ResourceSet::new(vec![
        ResourceClaim {
            id: ResourceId::from_bytes([1; 16]).unwrap(),
            mode: AccessMode::Exclusive,
        },
        ResourceClaim {
            id: ResourceId::from_bytes([2; 16]).unwrap(),
            mode: AccessMode::Exclusive,
        },
    ])
    .unwrap();
    for (i, profile_path) in [
        "CAROSIGN_TEST_PROFILE_FIRST",
        "CAROSIGN_TEST_PROFILE_SECOND",
    ]
    .into_iter()
    .enumerate()
    {
        assert!(store.fences().unwrap().is_empty());
        let resources = Arc::new(
            crate::resources::LocalResourceOwner::from_shared_fence_store(store.clone()).unwrap(),
        );
        let profile =
            ProviderProfile::open(Path::new(&std::env::var_os(profile_path).unwrap())).unwrap();
        assert!(profile.has_exclusive_owner_contract() && profile.requires_durable_binding());
        let mut supervisor =
            ProviderSupervisor::start(&profile, boot, DeviceEpoch::FIRST, Duration::from_secs(10))
                .unwrap();
        let process_status =
            std::fs::read_to_string(format!("/proc/{}/status", supervisor.worker.child_id()))
                .unwrap();
        for (field, expected) in [
            ("Uid:", profile.provider_uid),
            ("Gid:", profile.provider_gid),
        ] {
            let values: Vec<u32> = process_status
                .lines()
                .find_map(|line| line.strip_prefix(field))
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            assert!(
                values.iter().take(3).all(|value| *value == expected),
                "неверные права worker"
            );
        }
        for field in ["CapInh:", "CapPrm:", "CapEff:", "CapAmb:"] {
            let value = process_status
                .lines()
                .find_map(|line| line.strip_prefix(field))
                .unwrap()
                .trim();
            assert_eq!(u64::from_str_radix(value, 16).unwrap(), 0, "{field}");
        }
        assert!(
            process_status
                .lines()
                .any(|line| line.split_whitespace().eq(["NoNewPrivs:", "1"]))
        );
        let owner = ResourceOwner {
            operation: OperationId::from_bytes([3 + i as u8; 16]).unwrap(),
            worker: supervisor.worker_id(),
            authenticator: AuthenticatorRef {
                id: profile.identity.authenticator,
                device_epoch: DeviceEpoch::FIRST,
            },
        };
        let lease = futures_executor::block_on(resources.acquire_for(owner, &set)).unwrap();
        let scope = crate::resources::CleanupScope::of(&lease);
        let (transport, capabilities, _) = crate::supervised_verification::prepare_mutation_tunnel(
            &mut supervisor,
            &resources,
            &scope,
            *profile.identity.package.as_bytes(),
            *profile.identity.profile.as_bytes(),
            Duration::from_secs(30),
            false,
        )
        .expect("реальная привязка Device");
        assert_eq!(capabilities.modality, profile.identity.modality as i32);
        assert!(
            capabilities.operations.is_empty(),
            "проба не заявляет биометрические операции"
        );
        assert_eq!(
            store.fences().unwrap(),
            set.claims()
                .iter()
                .map(|claim| claim.id)
                .collect::<Vec<_>>(),
            "durable fence должен существовать во время lease"
        );
        eprintln!("lease {}: Bind выполнен, durable fence записан", i + 1);
        let scheduler = resources.scheduler();
        let held = scheduler
            .lock()
            .unwrap()
            .begin_quiescing(lease.lease_id(), owner, &set)
            .unwrap();
        let cleanup = crate::resources::SupervisorCleanup::new_for_scope(
            supervisor,
            resources.clone(),
            scope.clone(),
        );
        let proof = futures_executor::block_on(confirm_quiesced_for(&cleanup, &scope, &held))
            .expect("scoped CANCEL/revoke");
        let released = futures_executor::block_on(confirm_released(&cleanup, &scope, proof))
            .expect("подтверждение освобождения");
        scheduler
            .lock()
            .unwrap()
            .finish_quiescing(held, released)
            .ok()
            .expect("durable release");
        drop(transport);
        drop(cleanup);
        drop(scheduler);
        drop(resources);
        drop(store);
        store = Arc::new(InitialFenceStore::open_guarded(&path).expect("повторное открытие redb"));
        assert!(store.fences().unwrap().is_empty(), "fence не снят");
        eprintln!(
            "lease {}: scoped CANCEL/revoke и снятие fence подтверждены",
            i + 1
        );
    }
}
