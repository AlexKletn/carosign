//! Testing a real D-Bus adapter with a Linux component and a mock provider.

#[path = "linux_service_adapters_tests_fixtures/child_guard.rs"]
mod child_guard;
#[path = "linux_service_adapters_tests_fixtures/pam_conv.rs"]
mod pam_conv;

use std::process::{Child, Command};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, EnrollmentId};
use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
use carosign_persistence::mutation_plan::StoredBindingKind;
use carosign_persistence::store::{MetadataStore, Table};
use dbus::{blocking::Connection, message::MatchRule};

use child_guard::ChildGuard;

#[test]
#[ignore = "запускать tools/test-linux-adapters.sh в изолированных пространствах"]
fn real_fprintd_and_linux_service_with_mock() {
    assert_eq!(
        std::env::var("CAROSIGN_TEST_PRIVATE_RUN").as_deref(),
        Ok("1")
    );
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let dir = std::path::Path::new("/run/carosignd");
    std::fs::create_dir_all(dir).unwrap();
    let policy_path = dir.join("policy");
    std::fs::write(
        &policy_path,
        "uid=0 subject=0 actions=verify purpose=session-login\n",
    )
    .unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&dir.join("store.redb"))
            .unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes(*b"linux-local-v001").unwrap(),
        subject: carosign_adapters::policy::subject_of(0),
    };
    let authenticator = AuthenticatorId::from_bytes([5; 16]).unwrap();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([15; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment,
        display_label: None,
        label: "right-index-finger".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![21; 16]).unwrap(),
        format_id: String::new(),
        format_version: 0,
        binding_kind: StoredBindingKind::ProviderOpaque,
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
        0,
        carosign_runtime::ports::TransactionId::from_bytes([2; 16]).unwrap(),
    );
    let intent = carosign_runtime::authority::verification::VerificationIntent {
        report_matched_enrollment: false,
        enrollment: None,
        subject: carosign_runtime::authority::verification::RequestedSubject::AccountHint(
            "root".into(),
        ),
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
            carosign_core::device::DeviceEpoch::FIRST,
        ))
        .is_ok()
    );
    let profile = crate::provider_supervisor::ProviderProfile::fixture(
        std::env::var_os("CAROSIGN_TEST_BROKER").unwrap().into(),
        std::env::var_os("CAROSIGN_TEST_PROVIDER").unwrap().into(),
    );
    let config = crate::config::Config::default();
    std::thread::spawn(move || {
        crate::linux_service::serve(&config, &store, &policy, &profile, clock).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.join("control.sock").exists() {
        assert!(Instant::now() < deadline, "демон не открыл сокет");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Registration rights are verified separately from Verify: without them, denial.
    {
        use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
        let client = carosign_linux_ipc::client::transport::Transport::connect_root(
            "/run/carosignd/control.sock",
        )
        .unwrap();
        let response = client
            .request(wire::client_frame::Payload::BeginEnrollment(
                wire::BeginEnrollment {
                    idempotency_key: vec![99; 16],
                    subject: Some(common::SubjectSelector {
                        selector: Some(common::subject_selector::Selector::AccountHint(
                            "root".into(),
                        )),
                    }),
                    authenticator_id: vec![5; 16],
                    label: "left-thumb".into(),
                    replace: None,
                    authority_request_ref: vec![],
                },
            ))
            .unwrap();
        assert!(
            matches!(response.payload, Some(wire::client_frame::Payload::ProtocolError(error))
            if error.error.as_ref().is_some_and(|error| error.code == common::ErrorCode::ErrorUnauthorized as i32))
        );
    }
    let pending_case =
        std::env::var("CAROSIGN_TEST_SCENARIO").as_deref() == Ok("linux-pending-cancel");
    let no_sample = std::env::var("CAROSIGN_TEST_SCENARIO").as_deref() == Ok("linux-no-sample");
    if !pending_case {
        // Call the real libpam and the public point of the module through the system socket.
        use pam_conv::PamConv;
        let library = unsafe { libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW) };
        assert!(!library.is_null());
        let start_ptr = unsafe { libc::dlsym(library, c"pam_start".as_ptr()) };
        let end_ptr = unsafe { libc::dlsym(library, c"pam_end".as_ptr()) };
        assert!(!start_ptr.is_null() && !end_ptr.is_null());
        type PamStart = unsafe extern "C" fn(
            *const libc::c_char,
            *const libc::c_char,
            *const PamConv,
            *mut *mut libc::c_void,
        ) -> libc::c_int;
        type PamEnd = unsafe extern "C" fn(*mut libc::c_void, libc::c_int) -> libc::c_int;
        let start: PamStart = unsafe { core::mem::transmute(start_ptr) };
        let end: PamEnd = unsafe { core::mem::transmute(end_ptr) };
        let conversation = PamConv {
            conv: None,
            appdata_ptr: core::ptr::null_mut(),
        };
        let mut handle = core::ptr::null_mut();
        assert_eq!(
            unsafe {
                start(
                    c"carosign-test".as_ptr(),
                    c"root".as_ptr(),
                    &conversation,
                    &mut handle,
                )
            },
            0
        );
        let result = pam_carosign::pam_ffi::pam_sm_authenticate(handle, 0, 0, core::ptr::null());
        assert_eq!(
            result,
            if no_sample { 9 } else { 0 },
            "неверный результат PAM"
        );
        assert_eq!(unsafe { end(handle, result) }, 0);
        unsafe { libc::dlclose(library) };
    }
    let _adapter = ChildGuard(
        Command::new(std::env::var_os("CAROSIGN_TEST_FPRINTD").unwrap())
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
            )
            .spawn()
            .unwrap(),
    );
    let client = Connection::new_session().unwrap();
    let manager = client.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Manager",
        Duration::from_secs(5),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok((devices,)) = manager.method_call::<(Vec<dbus::Path<'static>>,), _, _, _>(
            "net.reactivated.Fprint.Manager",
            "GetDevices",
            (),
        ) {
            assert_eq!(devices.len(), 1);
            break;
        }
        assert!(Instant::now() < deadline, "адаптер не открыл шину");
        std::thread::sleep(Duration::from_millis(10));
    }
    let device = client.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Device/0",
        Duration::from_secs(5),
    );
    let interface = "net.reactivated.Fprint.Device";
    device
        .method_call::<(), _, _, _>(interface, "Claim", ("root",))
        .unwrap();
    let (fingers,): (Vec<String>,) = device
        .method_call(interface, "ListEnrolledFingers", ("root",))
        .unwrap();
    assert_eq!(fingers, ["right-index-finger"]);
    let (enroll_tx, enroll_rx) = mpsc::channel();
    client
        .add_match(
            MatchRule::new_signal(interface, "EnrollStatus"),
            move |event: (String, bool), _, _| {
                enroll_tx.send(event).unwrap();
                true
            },
        )
        .unwrap();
    device
        .method_call::<(), _, _, _>(interface, "EnrollStart", ("left-thumb",))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(event) = enroll_rx.try_recv() {
            assert_eq!(event, ("enroll-failed".into(), true));
            break;
        }
        assert!(Instant::now() < deadline, "отказ регистрации не доставлен");
        client.process(Duration::from_millis(20)).unwrap();
    }
    device
        .method_call::<(), _, _, _>(interface, "EnrollStop", ())
        .unwrap();
    let error = device
        .method_call::<(), _, _, _>(interface, "DeleteEnrolledFingers2", ())
        .unwrap_err();
    assert_eq!(
        error.name(),
        Some("net.reactivated.Fprint.Error.PrintsNotDeleted")
    );
    let (tx, rx) = mpsc::channel();
    client
        .add_match(
            MatchRule::new_signal(interface, "VerifyStatus"),
            move |args: (String, bool), _, _| {
                tx.send(args).unwrap();
                true
            },
        )
        .unwrap();
    if pending_case {
        device
            .method_call::<(), _, _, _>(interface, "VerifyStart", ("any",))
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            rx.try_recv().is_err(),
            "ожидающая проверка преждевременно завершилась"
        );
        let started = Instant::now();
        let (devices,): (Vec<dbus::Path<'static>>,) = manager
            .method_call("net.reactivated.Fprint.Manager", "GetDevices", ())
            .unwrap();
        assert_eq!(devices.len(), 1, "долгая проверка скрыла устройство");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "обнаружение устройства зависло"
        );
        device
            .method_call::<(), _, _, _>(interface, "VerifyStop", ())
            .unwrap();
        device
            .method_call::<(), _, _, _>(interface, "Release", ())
            .unwrap();
        return;
    }
    for _ in 0..2 {
        device
            .method_call::<(), _, _, _>(interface, "VerifyStart", ("any",))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let result = loop {
            if let Ok(value) = rx.try_recv() {
                break value;
            }
            assert!(Instant::now() < deadline, "проверка не завершилась");
            client.process(Duration::from_millis(20)).unwrap();
        };
        assert_eq!(
            result,
            (
                if no_sample {
                    "verify-unknown-error"
                } else {
                    "verify-match"
                }
                .to_owned(),
                true
            )
        );
        device
            .method_call::<(), _, _, _>(interface, "VerifyStop", ())
            .unwrap();
    }
    device
        .method_call::<(), _, _, _>(interface, "Release", ())
        .unwrap();
    if no_sample {
        return;
    }
    let output = Command::new("fprintd-verify")
        .args(["-f", "any", "root"])
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        )
        .output()
        .expect("для испытания нужен fprintd-verify");
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("verify-match"),
        "fprintd-verify: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
