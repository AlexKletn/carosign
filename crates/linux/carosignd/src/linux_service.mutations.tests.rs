//! End-to-end registration, inspection and disposal with a stable moss state.

use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wire::client_frame::Payload;

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "запускать tools/test-linux-mutation.sh в изолированных пространствах"]
fn enroll_verify_remove_with_separate_mock_processes() {
    assert_eq!(std::env::var("CAROSIGN_TEST_PRIVATE_RUN").as_deref(), Ok("1"));
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let dir = std::path::Path::new("/run/carosignd");
    std::fs::create_dir_all(dir).unwrap();
    let policy_path = dir.join("policy");
    std::fs::write(
        &policy_path,
        "uid=0 subject=0 actions=verify,enroll,remove,inventory purpose=session-login\n",
    )
    .unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&dir.join("store.redb"))
            .unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&policy_path, clock.clone());
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
    let selector = || {
        Some(common::SubjectSelector {
            selector: Some(common::subject_selector::Selector::AccountHint(
                "root".into(),
            )),
        })
    };
    use dbus::{blocking::Connection, message::MatchRule};
    let _adapter = ChildGuard(
        std::process::Command::new(std::env::var_os("CAROSIGN_TEST_FPRINTD").unwrap())
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
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let device = client.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Device/0",
        Duration::from_secs(10),
    );
    let interface = "net.reactivated.Fprint.Device";
    device
        .method_call::<(), _, _, _>(interface, "Claim", ("root",))
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    client
        .add_match(
            MatchRule::new_signal(interface, "EnrollStatus"),
            move |event: (String, bool), _, _| {
                tx.send(event).unwrap();
                true
            },
        )
        .unwrap();
    device
        .method_call::<(), _, _, _>(interface, "EnrollStart", ("left-thumb",))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(event) = rx.try_recv() {
            assert_eq!(event, ("enroll-completed".into(), true));
            break;
        }
        assert!(Instant::now() < deadline, "регистрация не завершилась");
        client.process(Duration::from_millis(20)).unwrap();
    }
    device
        .method_call::<(), _, _, _>(interface, "EnrollStop", ())
        .unwrap();
    let (fingers,): (Vec<String>,) = device
        .method_call(interface, "ListEnrolledFingers", ("root",))
        .unwrap();
    assert_eq!(fingers, ["left-thumb"]);
    let transport = Transport::connect_root("/run/carosignd/control.sock").unwrap();
    let list = || {
        let response = transport.request(Payload::ListEnrollments(wire::ListEnrollments {
            subject: selector(),
            authenticator_id: vec![5; 16],
            authority_request_ref: vec![],
        })).unwrap();
        let Some(Payload::Enrollments(list)) = response.payload else { panic!("missing enrollments") };
        assert_eq!(list.entries.len(), 1);
        list.entries.into_iter().next().unwrap()
    };
    let original = list();
    let reference = original.enrollment.unwrap();
    let rename = |reference, expected: &str, label: &str| {
        transport.request(Payload::RenameEnrollment(wire::RenameEnrollment {
            enrollment: Some(reference),
            expected_label: Some(expected.to_owned()),
            label: label.to_owned(),
        })).unwrap().payload
    };
    let mut stale = reference.clone();
    stale.generation += 1;
    for (target, expected) in [(stale, "left-thumb"), (reference.clone(), "stale name")] {
        assert!(matches!(
            rename(target, expected, "My thumb"),
            Some(Payload::ProtocolError(error))
                if error.error.as_ref().is_some_and(|error| error.code() == common::ErrorCode::ErrorGenerationConflict)
        ));
    }
    for _ in 0..2 {
        assert!(matches!(
            rename(reference.clone(), "left-thumb", "My thumb"),
            Some(Payload::EnrollmentRenamed(renamed))
                if renamed.enrollment.as_ref() == Some(&reference) && renamed.label == "My thumb"
        ));
    }
    let renamed = list();
    assert_eq!(renamed.enrollment, Some(reference));
    assert_eq!(renamed.label, "My thumb");
    assert_eq!(renamed.original_label.as_deref(), Some("left-thumb"));
    let (fingers,): (Vec<String>,) = device
        .method_call(interface, "ListEnrolledFingers", ("root",))
        .unwrap();
    assert_eq!(fingers, ["left-thumb"]);
    drop(transport);
    let verification = pam_carosign::client::observe_system_verification(
        "/run/carosignd/control.sock",
        &pam_carosign::client::Request {
            user: "root".into(),
            service: String::new(),
            connection: String::new(),
        },
        [72; 16],
        common::VerificationPurpose::PurposeSessionLogin,
    )
    .unwrap();
    assert_eq!(
        verification,
        pam_carosign::client::Observation::FactorAccepted
    );
    device
        .method_call::<(), _, _, _>(interface, "VerifyStart", ("any",))
        .unwrap();
    device
        .method_call::<(), _, _, _>(interface, "VerifyStop", ())
        .unwrap();
    device
        .method_call::<(), _, _, _>(interface, "DeleteEnrolledFingers2", ())
        .unwrap();
    device
        .method_call::<(), _, _, _>(interface, "Release", ())
        .unwrap();
    let transport = Transport::connect_root("/run/carosignd/control.sock").unwrap();
    let response = transport
        .request(Payload::ListEnrollments(wire::ListEnrollments {
            subject: selector(),
            authenticator_id: vec![5; 16],
            authority_request_ref: vec![],
        }))
        .unwrap();
    assert!(
        matches!(response.payload, Some(Payload::Enrollments(ref list)) if list.entries.is_empty())
    );
    drop(transport);
    for (program, args, expected) in [
        (
            "fprintd-enroll",
            vec!["-f", "left-thumb", "root"],
            "enroll-completed",
        ),
        ("fprintd-verify", vec!["-f", "any", "root"], "verify-match"),
        ("fprintd-delete", vec!["root"], ""),
    ] {
        let output = std::process::Command::new(program)
            .args(args)
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains(expected),
            "{program}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let transport = Transport::connect_root("/run/carosignd/control.sock").unwrap();
    let response = transport
        .request(Payload::ListEnrollments(wire::ListEnrollments {
            subject: selector(),
            authenticator_id: vec![5; 16],
            authority_request_ref: vec![],
        }))
        .unwrap();
    assert!(
        matches!(response.payload, Some(Payload::Enrollments(ref list)) if list.entries.is_empty()),
        "fprintd-delete не удалил регистрацию"
    );
    let capture = carosign_linux_ipc::client::mutation_events::observe_mutation_with_acceptance(
        &transport,
        Payload::BeginEnrollment(wire::BeginEnrollment {
            idempotency_key: vec![84; 16],
            authenticator_id: vec![5; 16],
            subject: selector(),
            label: String::new(),
            ..Default::default()
        }),
        |_| {},
    )
    .unwrap();
    assert_eq!(capture.result(), wire::MutationResult::MutationCommitted);
    let captured = capture.enrollment.unwrap();
    let response = transport.request(Payload::RenameEnrollment(wire::RenameEnrollment {
        enrollment: Some(captured.clone()),
        expected_label: Some(String::new()),
        label: "Name after scanning".into(),
    })).unwrap();
    assert!(matches!(response.payload, Some(Payload::EnrollmentRenamed(ref renamed))
        if renamed.enrollment.as_ref() == Some(&captured) && renamed.label == "Name after scanning"));
    let removal = carosign_linux_ipc::client::mutation_events::observe_mutation_with_acceptance(
        &transport,
        Payload::BeginRemoval(wire::BeginRemoval {
            idempotency_key: vec![85; 16],
            enrollment: Some(captured),
            subject: selector(),
            ..Default::default()
        }),
        |_| {},
    ).unwrap();
    assert_eq!(removal.result(), wire::MutationResult::MutationCommitted);
}
