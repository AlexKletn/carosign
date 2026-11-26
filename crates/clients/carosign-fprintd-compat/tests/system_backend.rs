//! Exercises the adapter with a private bus and a separate mock.

use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
use dbus::{blocking::Connection, message::MatchRule};
use prost::Message;
use std::os::{
    fd::{AsRawFd, FromRawFd, OwnedFd},
    unix::process::CommandExt,
};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use wire::client_frame::Payload;

const SOCKET: &str = "/run/carosignd/control.sock";
const DEVICE: &str = "net.reactivated.Fprint.Device";
const PATH: &str = "/net/reactivated/Fprint/Device/0";

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn read(fd: i32) -> Option<wire::ClientFrame> {
    let mut bytes = [0u8; 65536];
    let count = unsafe { libc::recv(fd, bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    (count > 0).then(|| wire::ClientFrame::decode(&bytes[..count as usize]).unwrap())
}

fn write(fd: i32, frame: wire::ClientFrame) {
    let bytes = frame.encode_to_vec();
    assert_eq!(
        unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) },
        bytes.len() as isize
    );
}

fn response(minor: u32, request_id: Vec<u8>, payload: Payload) -> wire::ClientFrame {
    wire::ClientFrame {
        major: 1,
        minor,
        request_id,
        operation_id: vec![],
        event_sequence: 0,
        payload: Some(payload),
    }
}

fn terminal(fd: i32, minor: u32, operation: &[u8], result: wire::VerificationResult) {
    write(
        fd,
        wire::ClientFrame {
            major: 1,
            minor,
            request_id: vec![],
            operation_id: operation.to_vec(),
            event_sequence: 1,
            payload: Some(Payload::OperationEvent(wire::OperationEvent {
                capture_state: None,
                feedback: None,
                operation_id: operation.to_vec(),
                sequence: 1,
                kind: wire::EventKind::EventTerminal as i32,
                phase: wire::OperationPhase::OpTerminal as i32,
                percent: 0,
                gap_after_sequence: 0,
                snapshot: None,
                terminal: Some(wire::TerminalResult {
                    kind: Some(wire::terminal_result::Kind::Verification(
                        wire::VerificationTerminal {
                            matched_enrollment: None,
                            result: result as i32,
                            error: None,
                        },
                    )),
                }),
            })),
        },
    );
}

fn serve_client(fd: OwnedFd) {
    let Some(hello) = read(fd.as_raw_fd()) else {
        return;
    };
    let Some(Payload::Hello(body)) = hello.payload else {
        return;
    };
    let minor = body.max_minor;
    write(
        fd.as_raw_fd(),
        response(
            minor,
            hello.request_id,
            Payload::HelloAck(wire::HelloAck {
                verification_enrollment_selector: false,
                capture_feedback_v1: false,
                major: 1,
                selected_minor: minor,
                connection_id: vec![1; 16],
            }),
        ),
    );
    while let Some(frame) = read(fd.as_raw_fd()) {
        match frame.payload {
            Some(Payload::ListAuthenticators(_)) => {
                if std::fs::read_to_string("/run/carosignd/mode")
                    .is_ok_and(|mode| mode == "disconnect")
                {
                    // Inject a transport failure after Hello to distinguish it from an empty inventory.
                    return;
                }
                write(
                    fd.as_raw_fd(),
                    response(
                        minor,
                        frame.request_id,
                        Payload::Authenticators(wire::Authenticators {
                            providers: None,
                            entries: vec![common::AuthenticatorDescriptor {
                                authenticator_id: vec![2; 16],
                                device_epoch: 1,
                                unavailability_reason: 0,
                                available: std::fs::read_to_string("/run/carosignd/mode")
                                    .unwrap_or_default()
                                    != "offline",
                                capabilities: Some(common::CapabilityDescriptor {
                                    modality: common::Modality::Fingerprint as i32,
                                    enrollment_stages: Some(3),
                                    operations: vec![
                                        common::ProviderOperation::ProviderVerify as i32,
                                        common::ProviderOperation::ProviderEnroll as i32,
                                    ],
                                    ..Default::default()
                                }),
                            }],
                        }),
                    ),
                );
            }
            Some(Payload::ListEnrollments(_)) => write(
                fd.as_raw_fd(),
                response(
                    minor,
                    frame.request_id,
                    Payload::Enrollments(wire::Enrollments { entries: vec![] }),
                ),
            ),
            Some(Payload::BeginVerification(begin)) => {
                assert!(
                    matches!(begin.subject.and_then(|s| s.selector), Some(common::subject_selector::Selector::AccountHint(name)) if name == "root")
                );
                let operation = vec![3; 16];
                let mut accepted = response(
                    minor,
                    frame.request_id,
                    Payload::OperationAccepted(wire::OperationAccepted {
                        operation_id: operation.clone(),
                        first_event_sequence: 1,
                        deadline: Some(common::BootDeadline {
                            boot_id: vec![4; 16],
                            boottime_ns: 1000,
                        }),
                    }),
                );
                accepted.operation_id = operation.clone();
                write(fd.as_raw_fd(), accepted);
                match std::fs::read_to_string("/run/carosignd/mode")
                    .unwrap()
                    .as_str()
                {
                    "match" => terminal(
                        fd.as_raw_fd(),
                        minor,
                        &operation,
                        wire::VerificationResult::VerifyFactorAccepted,
                    ),
                    "deny" => terminal(
                        fd.as_raw_fd(),
                        minor,
                        &operation,
                        wire::VerificationResult::VerifyNotMatched,
                    ),
                    "cancel" => {
                        let cancel = read(fd.as_raw_fd()).unwrap();
                        assert!(
                            matches!(cancel.payload, Some(Payload::CancelOperation(ref c)) if c.operation_id == operation)
                        );
                        write(
                            fd.as_raw_fd(),
                            response(
                                minor,
                                cancel.request_id,
                                Payload::CancellationReceipt(wire::CancellationReceipt {
                                    result: wire::CancelReceiptKind::CancelReceiptRequested as i32,
                                }),
                            ),
                        );
                        std::fs::write("/run/carosignd/cancelled", b"yes").unwrap();
                        terminal(
                            fd.as_raw_fd(),
                            minor,
                            &operation,
                            wire::VerificationResult::VerifyCancelled,
                        );
                    }
                    _ => panic!("неизвестный сценарий mock"),
                }
            }
            Some(Payload::BeginEnrollment(begin)) => {
                assert_eq!(begin.label, "left-thumb");
                assert!(
                    matches!(begin.subject.and_then(|s| s.selector), Some(common::subject_selector::Selector::AccountHint(name)) if name == "root")
                );
                let operation = vec![9; 16];
                let mut accepted = response(
                    minor,
                    frame.request_id,
                    Payload::OperationAccepted(wire::OperationAccepted {
                        operation_id: operation.clone(),
                        first_event_sequence: 1,
                        deadline: Some(common::BootDeadline {
                            boot_id: vec![4; 16],
                            boottime_ns: 1000,
                        }),
                    }),
                );
                accepted.operation_id = operation.clone();
                write(fd.as_raw_fd(), accepted);
                if std::fs::read_to_string("/run/carosignd/mode")
                    .is_ok_and(|mode| mode == "cancel-enrollment")
                {
                    let cancel = read(fd.as_raw_fd()).unwrap();
                    assert!(
                        matches!(cancel.payload, Some(Payload::CancelOperation(ref c)) if c.operation_id == operation)
                    );
                    std::fs::write("/run/carosignd/mode", "offline").unwrap();
                    write(
                        fd.as_raw_fd(),
                        response(
                            minor,
                            cancel.request_id,
                            Payload::CancellationReceipt(wire::CancellationReceipt {
                                result: wire::CancelReceiptKind::CancelReceiptRequested as i32,
                            }),
                        ),
                    );
                    return;
                }
                write(
                    fd.as_raw_fd(),
                    wire::ClientFrame {
                        major: 1,
                        minor,
                        request_id: vec![],
                        operation_id: operation.clone(),
                        event_sequence: 1,
                        payload: Some(Payload::OperationEvent(wire::OperationEvent {
                            feedback: None,
                            operation_id: operation,
                            sequence: 1,
                            kind: wire::EventKind::EventTerminal as i32,
                            phase: wire::OperationPhase::OpTerminal as i32,
                            terminal: Some(wire::TerminalResult {
                                kind: Some(wire::terminal_result::Kind::Mutation(
                                    wire::MutationTerminal {
                                        result: wire::MutationResult::MutationCommitted as i32,
                                        enrollment: Some(common::EnrollmentRef {
                                            enrollment_id: vec![8; 16],
                                            authenticator_id: begin.authenticator_id,
                                            generation: 1,
                                        }),
                                        mutation_id: vec![7; 16],
                                        error: None,
                                    },
                                )),
                            }),
                            ..Default::default()
                        })),
                    },
                );
            }
            _ => return,
        }
    }
}

#[test]
#[ignore = "отдельный mock-процесс для изолированного испытания"]
fn mock_daemon() {
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0) };
    assert!(raw >= 0);
    let listener = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as _;
    for (dst, src) in address.sun_path.iter_mut().zip(SOCKET.as_bytes()) {
        *dst = *src as _;
    }
    let len =
        (core::mem::offset_of!(libc::sockaddr_un, sun_path) + SOCKET.len() + 1) as libc::socklen_t;
    assert_eq!(
        unsafe { libc::bind(raw, (&address as *const libc::sockaddr_un).cast(), len) },
        0
    );
    assert_eq!(unsafe { libc::listen(raw, 16) }, 0);
    loop {
        let peer = unsafe {
            libc::accept(
                listener.as_raw_fd(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        assert!(peer >= 0);
        std::thread::spawn(move || serve_client(unsafe { OwnedFd::from_raw_fd(peer) }));
    }
}

#[test]
#[ignore = "должен запускаться внутри приватных user/mount namespaces и отдельной D-Bus шины"]
fn real_system_backend_scenario() {
    assert_eq!(
        std::env::var("CAROSIGN_TEST_PRIVATE_RUN").as_deref(),
        Ok("1")
    );
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let executable = std::env::current_exe().unwrap();
    let _mock = Guard(
        Command::new(&executable)
            .args(["--exact", "mock_daemon", "--ignored"])
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !std::path::Path::new(SOCKET).exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let diagnostic_log = "/run/carosignd/fprintd-diagnostic.log";
    let _adapter = Guard(
        Command::new(env!("CARGO_BIN_EXE_carosign-fprintd-compat"))
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
            )
            .env("CAROSIGN_FPRINT_DIAGNOSTICS", "1")
            .stderr(std::fs::File::create(diagnostic_log).unwrap())
            .spawn()
            .unwrap(),
    );
    let client = Connection::new_session().unwrap();
    let manager = client.with_proxy(
        "net.reactivated.Fprint",
        "/net/reactivated/Fprint/Manager",
        Duration::from_secs(2),
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
    let device = client.with_proxy("net.reactivated.Fprint", PATH, Duration::from_secs(2));
    let proxy_fixture = "/run/carosignd/gnome-proxy";
    let flags = Command::new("pkg-config")
        .args(["--cflags", "--libs", "gio-2.0"])
        .output()
        .unwrap();
    assert!(flags.status.success());
    let flags = String::from_utf8(flags.stdout).unwrap();
    let mut compiler = Command::new("cc");
    compiler.args([
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/gnome_proxy.c"),
        "-o",
        proxy_fixture,
    ]);
    compiler.args(flags.split_whitespace());
    assert!(compiler.status().unwrap().success());
    assert!(
        Command::new(proxy_fixture).status().unwrap().success(),
        "GDBusProxy не получил свойства"
    );
    std::fs::write("/run/carosignd/mode", "disconnect").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    manager
        .method_call::<(dbus::Path<'static>,), _, _, _>(
            "net.reactivated.Fprint.Manager",
            "GetDefaultDevice",
            (),
        )
        .expect("краткий обрыв IPC не должен скрывать устройство");
    std::fs::write("/run/carosignd/mode", "match").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    manager
        .method_call::<(dbus::Path<'static>,), _, _, _>(
            "net.reactivated.Fprint.Manager",
            "GetDefaultDevice",
            (),
        )
        .unwrap();
    std::fs::write("/run/carosignd/mode", "offline").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    let (device_path,): (dbus::Path<'static>,) = manager
        .method_call("net.reactivated.Fprint.Manager", "GetDefaultDevice", ())
        .expect("датчик остаётся в инвентаре при недоступности операций");
    assert_eq!(device_path.to_string(), PATH);
    let (_properties,): (dbus::arg::PropMap,) = device
        .method_call("org.freedesktop.DBus.Properties", "GetAll", (DEVICE,))
        .unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    let (device_path,): (dbus::Path<'static>,) = manager
        .method_call("net.reactivated.Fprint.Manager", "GetDefaultDevice", ())
        .expect("ограждение не должно скрывать датчик по истечении grace");
    assert_eq!(device_path.to_string(), PATH);
    std::fs::write("/run/carosignd/mode", "disconnect").unwrap();
    std::thread::sleep(Duration::from_millis(2100));
    let unavailable = manager
        .method_call::<(dbus::Path<'static>,), _, _, _>(
            "net.reactivated.Fprint.Manager",
            "GetDefaultDevice",
            (),
        )
        .unwrap_err();
    assert_eq!(
        unavailable.name(),
        Some("net.reactivated.Fprint.Error.NoSuchDevice")
    );
    std::fs::write("/run/carosignd/mode", "match").unwrap();
    std::thread::sleep(Duration::from_millis(600));
    manager
        .method_call::<(dbus::Path<'static>,), _, _, _>(
            "net.reactivated.Fprint.Manager",
            "GetDefaultDevice",
            (),
        )
        .expect("устройство должно снова появиться после восстановления");
    let diagnostics = std::fs::read_to_string(diagnostic_log).unwrap();
    for event in [
        "get_default_device_request",
        "get_default_device_reply",
        "get_default_device_error",
        "get_all_request",
        "get_all_reply",
        "ipc_list_error",
        "availability_observed_absent",
        "availability_grace_retained",
        "availability_became_absent",
        "availability_became_present",
    ] {
        assert!(
            diagnostics.contains(event),
            "нет события {event}: {diagnostics}"
        );
    }
    assert!(diagnostics.lines().all(|line| {
        line.starts_with("fprintd-diag monotonic=")
            && !line.contains("root")
            && !line.contains("thumb")
    }));
    device
        .method_call::<(), _, _, _>(DEVICE, "Claim", ("root",))
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    client
        .add_match(
            MatchRule::new_signal(DEVICE, "VerifyStatus"),
            move |(value, done): (String, bool), _, _| {
                tx.send((value, done)).unwrap();
                true
            },
        )
        .unwrap();
    for (mode, expected) in [("match", "verify-match"), ("deny", "verify-no-match")] {
        std::fs::write("/run/carosignd/mode", mode).unwrap();
        device
            .method_call::<(), _, _, _>(DEVICE, "VerifyStart", ("any",))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let outcome = loop {
            if let Ok(value) = rx.try_recv() {
                break value;
            }
            assert!(Instant::now() < deadline, "результат не доставлен");
            client.process(Duration::from_millis(20)).unwrap();
        };
        assert_eq!(outcome, (expected.to_owned(), true));
        device
            .method_call::<(), _, _, _>(DEVICE, "VerifyStop", ())
            .unwrap();
    }
    let (enroll_tx, enroll_rx) = std::sync::mpsc::channel();
    client
        .add_match(
            MatchRule::new_signal(DEVICE, "EnrollStatus"),
            move |(value, done): (String, bool), _, _| {
                enroll_tx.send((value, done)).unwrap();
                true
            },
        )
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("left-thumb",))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let outcome = loop {
        if let Ok(value) = enroll_rx.try_recv() {
            break value;
        }
        assert!(Instant::now() < deadline, "итог регистрации не доставлен");
        client.process(Duration::from_millis(20)).unwrap();
    };
    assert_eq!(outcome, ("enroll-completed".to_owned(), true));
    device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStop", ())
        .unwrap();
    std::fs::write("/run/carosignd/mode", "cancel-enrollment").unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("left-thumb",))
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStop", ())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !std::fs::read_to_string("/run/carosignd/mode").is_ok_and(|mode| mode == "offline") {
        assert!(Instant::now() < deadline, "отмена регистрации не дошла");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(2100));
    manager
        .method_call::<(dbus::Path<'static>,), _, _, _>(
            "net.reactivated.Fprint.Manager",
            "GetDefaultDevice",
            (),
        )
        .expect("датчик должен остаться видимым после отмены регистрации");
    let unavailable = device
        .method_call::<(), _, _, _>(DEVICE, "EnrollStart", ("left-thumb",))
        .unwrap_err();
    assert_eq!(
        unavailable.name(),
        Some("net.reactivated.Fprint.Error.Internal")
    );
    std::fs::write("/run/carosignd/mode", "cancel").unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "VerifyStart", ("any",))
        .unwrap();
    device
        .method_call::<(), _, _, _>(DEVICE, "VerifyStop", ())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !std::path::Path::new("/run/carosignd/cancelled").exists() {
        assert!(Instant::now() < deadline, "отмена не дошла");
        std::thread::sleep(Duration::from_millis(10));
    }
    device
        .method_call::<(), _, _, _>(DEVICE, "Release", ())
        .unwrap();
    // Authorization must reject the other UID even when no operation is active.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if device
            .method_call::<(), _, _, _>(DEVICE, "Claim", ("root",))
            .is_ok()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "устройство осталось занятым после отмены"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    device
        .method_call::<(), _, _, _>(DEVICE, "Release", ())
        .unwrap();
    client.process(Duration::from_millis(100)).unwrap();
    assert!(
        rx.try_recv().is_err(),
        "отменённая проверка доставила результат"
    );
    let empty = device
        .method_call::<(Vec<String>,), _, _, _>(DEVICE, "ListEnrolledFingers", ("root",))
        .unwrap_err();
    assert_eq!(
        empty.name(),
        Some("net.reactivated.Fprint.Error.NoEnrolledPrints")
    );
    let foreign_executable = "/run/carosignd/foreign-fixture";
    std::fs::copy(&executable, foreign_executable).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(foreign_executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = Command::new(foreign_executable);
    command.args(["--exact", "foreign_user", "--ignored", "--nocapture"]);
    // SAFETY: The child only changes its UID before exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setuid(1) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let child = command.output().unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
#[ignore = "дочерний процесс с чужим UID на испытательной D-Bus шине"]
fn foreign_user() {
    assert_eq!(unsafe { libc::geteuid() }, 1);
    let client = Connection::new_session().unwrap();
    let error = client
        .request_name("net.reactivated.Fprint", false, false, true)
        .unwrap_err();
    assert_eq!(
        error.name(),
        Some("org.freedesktop.DBus.Error.AccessDenied")
    );
    let device = client.with_proxy("net.reactivated.Fprint", PATH, Duration::from_secs(2));
    let denied = device
        .method_call::<(Vec<String>,), _, _, _>(DEVICE, "ListEnrolledFingers", ("",))
        .unwrap_err();
    assert_eq!(
        denied.name(),
        Some("net.reactivated.Fprint.Error.PermissionDenied")
    );
    let error = device
        .method_call::<(Vec<String>,), _, _, _>(DEVICE, "ListEnrolledFingers", ("root",))
        .unwrap_err();
    assert_eq!(
        error.name(),
        Some("net.reactivated.Fprint.Error.PermissionDenied")
    );
    let error = device
        .method_call::<(), _, _, _>(DEVICE, "Claim", ("root",))
        .unwrap_err();
    assert_eq!(
        error.name(),
        Some("net.reactivated.Fprint.Error.PermissionDenied")
    );
}
