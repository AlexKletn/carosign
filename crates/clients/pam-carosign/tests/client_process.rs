//! The test process does not establish trust and does not allow PAM entry.

use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
use pam_carosign::client::{
    Observation, Request, observe_on_new_connection, observe_system_verification,
    observe_verification, observe_verification_with_acceptance,
};
use prost::Message;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use wire::{client_frame::Payload, terminal_result::Kind};

#[path = "client_process/pam_conv.rs"]
mod pam_conv;
#[path = "client_process/pam_message.rs"]
mod pam_message;
#[path = "client_process/pam_response.rs"]
mod pam_response;
use pam_conv::{PamConv, converse};

fn pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [0; 2];
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) },
        0
    );
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn read(fd: i32) -> wire::ClientFrame {
    let mut bytes = [0u8; 65536];
    let n = unsafe { libc::recv(fd, bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    assert!(n > 0);
    wire::ClientFrame::decode(&bytes[..n as usize]).unwrap()
}

fn write(fd: i32, frame: wire::ClientFrame) {
    let bytes = frame.encode_to_vec();
    assert_eq!(
        unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) },
        bytes.len() as isize
    );
}

#[test]
#[ignore = "служебный отдельный процесс испытания"]
fn fixture_mock_server() {
    let original: i32 = std::env::var("CAROSIGN_TEST_FD").unwrap().parse().unwrap();
    let fd = if std::env::var_os("CAROSIGN_TEST_LISTENER").is_some() {
        let accepted =
            unsafe { libc::accept(original, core::ptr::null_mut(), core::ptr::null_mut()) };
        assert!(accepted >= 0);
        accepted
    } else {
        original
    };
    let mode = std::env::var("CAROSIGN_TEST_RESULT").unwrap();
    let hello = read(fd);
    let Payload::Hello(hello_body) = hello.payload.unwrap() else {
        panic!("ожидалось приветствие")
    };
    let minor = hello_body.max_minor;
    write(
        fd,
        wire::ClientFrame {
            major: 1,
            minor,
            request_id: hello.request_id,
            operation_id: vec![],
            event_sequence: 0,
            payload: Some(Payload::HelloAck(wire::HelloAck {
                verification_enrollment_selector: false,
                capture_feedback_v1: false,
                major: 1,
                selected_minor: minor,
                connection_id: vec![7; 16],
            })),
        },
    );
    if std::env::var_os("CAROSIGN_TEST_LISTENER").is_some() {
        let list = read(fd);
        assert!(matches!(list.payload, Some(Payload::ListAuthenticators(_))));
        write(
            fd,
            wire::ClientFrame {
                major: 1,
                minor,
                request_id: list.request_id,
                operation_id: vec![],
                event_sequence: 0,
                payload: Some(Payload::Authenticators(wire::Authenticators {
                    providers: None,
                    entries: vec![common::AuthenticatorDescriptor {
                        authenticator_id: vec![4; 16],
                        device_epoch: 1,
                        available: true,
                        unavailability_reason: 0,
                        capabilities: Some(common::CapabilityDescriptor {
                            operations: vec![common::ProviderOperation::ProviderVerify as i32],
                            ..Default::default()
                        }),
                    }],
                })),
            },
        );
    }
    let begin = read(fd);
    assert!(matches!(begin.payload, Some(Payload::BeginVerification(_))));
    if std::env::var_os("CAROSIGN_TEST_LISTENER").is_some() {
        let Some(Payload::BeginVerification(ref request)) = begin.payload else {
            unreachable!()
        };
        assert!(
            matches!(request.subject.as_ref().and_then(|v| v.selector.as_ref()), Some(common::subject_selector::Selector::AccountHint(name)) if name == "alice")
        );
        assert_eq!(request.authenticator_id, vec![4; 16]);
    }
    let operation = vec![8; 16];
    write(
        fd,
        wire::ClientFrame {
            major: 1,
            minor,
            request_id: begin.request_id,
            operation_id: operation.clone(),
            event_sequence: 0,
            payload: Some(Payload::OperationAccepted(wire::OperationAccepted {
                operation_id: operation.clone(),
                first_event_sequence: 1,
                deadline: Some(common::BootDeadline {
                    boot_id: vec![9; 16],
                    boottime_ns: 1000,
                }),
            })),
        },
    );
    if mode == "disconnect" {
        return;
    }
    if mode == "cancel" {
        let cancel = read(fd);
        assert!(
            matches!(&cancel.payload, Some(Payload::CancelOperation(value)) if value.operation_id == operation)
        );
        write(
            fd,
            wire::ClientFrame {
                major: 1,
                minor,
                request_id: cancel.request_id,
                operation_id: vec![],
                event_sequence: 0,
                payload: Some(Payload::CancellationReceipt(wire::CancellationReceipt {
                    result: wire::CancelReceiptKind::CancelReceiptRequested as i32,
                })),
            },
        );
    }
    let (sequence, result) = match mode.as_str() {
        "accepted" => (1, wire::VerificationResult::VerifyFactorAccepted as i32),
        "not-matched" => (1, wire::VerificationResult::VerifyNotMatched as i32),
        "gap" => (2, wire::VerificationResult::VerifyFactorAccepted as i32),
        "unknown" => (1, 999),
        "cancel" => (1, wire::VerificationResult::VerifyCancelled as i32),
        _ => panic!("неизвестный режим"),
    };
    write(
        fd,
        wire::ClientFrame {
            major: 1,
            minor,
            request_id: vec![],
            operation_id: operation.clone(),
            event_sequence: sequence,
            payload: Some(Payload::OperationEvent(wire::OperationEvent {
                capture_state: None,
                feedback: None,
                operation_id: operation,
                sequence,
                kind: wire::EventKind::EventTerminal as i32,
                phase: wire::OperationPhase::OpTerminal as i32,
                percent: 0,
                gap_after_sequence: 0,
                snapshot: None,
                terminal: Some(wire::TerminalResult {
                    kind: Some(Kind::Verification(wire::VerificationTerminal {
                        matched_enrollment: None,
                        result,
                        error: None,
                    })),
                }),
            })),
        },
    );
}

#[test]
fn root_server_authenticates_and_binds_request() {
    // It is also run in the username space with the displayed root.
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    for mode in ["accepted", "not-matched", "gap"] {
        let path = std::env::temp_dir().join(format!("bio-pam-{}-{mode}.sock", std::process::id()));
        let listener = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0) };
        assert!(listener >= 0);
        let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as _;
        let bytes = path.as_os_str().as_encoded_bytes();
        assert!(bytes.len() < address.sun_path.len());
        for (dst, byte) in address.sun_path.iter_mut().zip(bytes) {
            *dst = *byte as _;
        }
        let len = (core::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1)
            as libc::socklen_t;
        assert_eq!(
            unsafe { libc::bind(listener, (&address as *const libc::sockaddr_un).cast(), len) },
            0
        );
        assert_eq!(unsafe { libc::listen(listener, 1) }, 0);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_mock_server", "--ignored"])
            .env("CAROSIGN_TEST_FD", listener.to_string())
            .env("CAROSIGN_TEST_LISTENER", "1")
            .env("CAROSIGN_TEST_RESULT", mode)
            .spawn()
            .unwrap();
        let request = Request {
            user: "alice".into(),
            service: "login".into(),
            connection: String::new(),
        };
        let result = observe_system_verification(
            path.to_str().unwrap(),
            &request,
            [2; 16],
            common::VerificationPurpose::PurposeSessionLogin,
        );
        match mode {
            "accepted" => assert_eq!(result.unwrap(), Observation::FactorAccepted),
            "not-matched" => assert_eq!(result.unwrap(), Observation::NotMatched),
            _ => assert!(result.is_err()),
        }
        assert!(child.wait().unwrap().success());
        unsafe { libc::close(listener) };
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "требует отдельное пользовательское и монтированное пространство имён"]
fn root_pam_entrypoint_against_server() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    assert_eq!(
        std::env::var("CAROSIGN_TEST_PRIVATE_RUN").as_deref(),
        Ok("1")
    );
    let path = "/run/carosignd/control.sock";
    let library = unsafe { libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW) };
    assert!(!library.is_null());
    type Start = unsafe extern "C" fn(
        *const libc::c_char,
        *const libc::c_char,
        *const PamConv,
        *mut *mut libc::c_void,
    ) -> libc::c_int;
    type End = unsafe extern "C" fn(*mut libc::c_void, libc::c_int) -> libc::c_int;
    let start: Start = unsafe { core::mem::transmute(libc::dlsym(library, c"pam_start".as_ptr())) };
    let end: End = unsafe { core::mem::transmute(libc::dlsym(library, c"pam_end".as_ptr())) };
    let tty_test = std::env::var_os("CAROSIGN_TEST_TTY").is_some();
    let silent_test = std::env::var_os("CAROSIGN_TEST_SILENT").is_some();
    let cases: &[(&str, i32)] = if tty_test {
        &[("cancel", 9)]
    } else if silent_test {
        &[("accepted", 0)]
    } else {
        &[("accepted", 0), ("not-matched", 7), ("gap", 9)]
    };
    for &(mode, expected) in cases {
        let listener = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0) };
        assert!(listener >= 0);
        let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as _;
        for (dst, byte) in address.sun_path.iter_mut().zip(path.as_bytes()) {
            *dst = *byte as _;
        }
        let len = (core::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + 1)
            as libc::socklen_t;
        assert_eq!(
            unsafe { libc::bind(listener, (&address as *const libc::sockaddr_un).cast(), len) },
            0
        );
        assert_eq!(unsafe { libc::listen(listener, 1) }, 0);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_mock_server", "--ignored"])
            .env("CAROSIGN_TEST_FD", listener.to_string())
            .env("CAROSIGN_TEST_LISTENER", "1")
            .env("CAROSIGN_TEST_RESULT", mode)
            .spawn()
            .unwrap();
        let mut prompts = 0usize;
        let conversation = PamConv {
            conv: Some(converse),
            appdata_ptr: (&mut prompts as *mut usize).cast(),
        };
        let mut handle = core::ptr::null_mut();
        assert_eq!(
            unsafe {
                start(
                    if tty_test {
                        c"sudo".as_ptr()
                    } else {
                        c"carosign-test".as_ptr()
                    },
                    c"alice".as_ptr(),
                    &conversation,
                    &mut handle,
                )
            },
            0
        );
        let result = pam_carosign::pam_ffi::pam_sm_authenticate(
            handle,
            if silent_test { 0x8000 } else { 0 },
            0,
            core::ptr::null(),
        );
        assert_eq!(result, expected);
        assert_eq!(prompts, if tty_test || silent_test { 0 } else { 1 });
        assert_eq!(unsafe { end(handle, result) }, 0);
        assert!(child.wait().unwrap().success());
        unsafe { libc::close(listener) };
        std::fs::remove_file(path).unwrap();
    }
    unsafe { libc::dlclose(library) };
}

#[test]
fn verification_exchange_with_untrusted_separate_process() {
    for mode in [
        "accepted",
        "not-matched",
        "gap",
        "unknown",
        "disconnect",
        "cancel",
    ] {
        let (client, server) = pair();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_mock_server", "--ignored"])
            .env("CAROSIGN_TEST_FD", server.as_raw_fd().to_string())
            .env("CAROSIGN_TEST_RESULT", mode)
            .spawn()
            .unwrap();
        drop(server);
        let transport = Transport::from_connected(client).unwrap();
        let request = wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![3; 16],
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: vec![4; 16],
            purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
            authority_request_ref: vec![],
        };
        let outcome = if mode == "cancel" {
            observe_verification_with_acceptance(&transport, request, |operation_id| {
                let response = transport
                    .request(Payload::CancelOperation(wire::CancelOperation {
                        operation_id: operation_id.to_vec(),
                    }))
                    .unwrap();
                assert!(matches!(
                    response.payload,
                    Some(Payload::CancellationReceipt(_))
                ));
            })
        } else {
            observe_verification(&transport, request)
        };
        match mode {
            "accepted" => assert_eq!(outcome.unwrap(), Observation::FactorAccepted),
            "not-matched" => assert_eq!(outcome.unwrap(), Observation::NotMatched),
            "cancel" => assert_eq!(outcome.unwrap(), Observation::Cancelled),
            _ => assert!(outcome.is_err()),
        }
        assert!(child.wait().unwrap().success());
    }
}

#[test]
fn concurrent_invocations_use_separate_connections_and_reject_crossed_results() {
    use std::os::unix::ffi::OsStrExt;
    use std::sync::{Arc, Barrier};

    let dir = std::env::temp_dir().join(format!("carosign-pam-b04-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("service.sock");
    let listener =
        unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(listener >= 0);
    let listener = unsafe { OwnedFd::from_raw_fd(listener) };
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let path_bytes = path.as_os_str().as_bytes();
    assert!(path_bytes.len() < addr.sun_path.len());
    for (dst, src) in addr.sun_path.iter_mut().zip(path_bytes) {
        *dst = *src as libc::c_char;
    }
    assert_eq!(
        unsafe {
            libc::bind(
                listener.as_raw_fd(),
                (&addr as *const libc::sockaddr_un).cast(),
                std::mem::size_of_val(&addr) as libc::socklen_t,
            )
        },
        0
    );
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 2) }, 0);

    let barrier = Arc::new(Barrier::new(2));
    let server = std::thread::spawn(move || {
        let mut workers = Vec::new();
        for _ in 0..2 {
            let fd = unsafe {
                libc::accept4(
                    listener.as_raw_fd(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    libc::SOCK_CLOEXEC,
                )
            };
            assert!(fd >= 0);
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                let hello = read(fd.as_raw_fd());
                let Payload::Hello(body) = hello.payload.unwrap() else {
                    panic!("ожидалось приветствие")
                };
                write(
                    fd.as_raw_fd(),
                    wire::ClientFrame {
                        major: 1,
                        minor: body.max_minor,
                        request_id: hello.request_id,
                        operation_id: vec![],
                        event_sequence: 0,
                        payload: Some(Payload::HelloAck(wire::HelloAck {
                            verification_enrollment_selector: false,
                            capture_feedback_v1: false,
                            major: 1,
                            selected_minor: body.max_minor,
                            connection_id: vec![9; 16],
                        })),
                    },
                );
                let begin = read(fd.as_raw_fd());
                let Some(Payload::BeginVerification(request)) = begin.payload else {
                    panic!("ожидалась проверка")
                };
                let id = request.idempotency_key[0];
                let operation = vec![id; 16];
                write(
                    fd.as_raw_fd(),
                    wire::ClientFrame {
                        major: 1,
                        minor: body.max_minor,
                        request_id: begin.request_id,
                        operation_id: operation.clone(),
                        event_sequence: 0,
                        payload: Some(Payload::OperationAccepted(wire::OperationAccepted {
                            operation_id: operation.clone(),
                            first_event_sequence: 1,
                            deadline: Some(common::BootDeadline {
                                boot_id: vec![9; 16],
                                boottime_ns: 1000,
                            }),
                        })),
                    },
                );
                barrier.wait();
                // For a second connection, someone else's operation should not be the result
                // of a call.
                let returned = if id == 2 { vec![1; 16] } else { operation };
                write(
                    fd.as_raw_fd(),
                    wire::ClientFrame {
                        major: 1,
                        minor: body.max_minor,
                        request_id: vec![],
                        operation_id: returned.clone(),
                        event_sequence: 1,
                        payload: Some(Payload::OperationEvent(wire::OperationEvent {
                            capture_state: None,
                            feedback: None,
                            operation_id: returned,
                            sequence: 1,
                            kind: wire::EventKind::EventTerminal as i32,
                            phase: wire::OperationPhase::OpTerminal as i32,
                            percent: 0,
                            gap_after_sequence: 0,
                            snapshot: None,
                            terminal: Some(wire::TerminalResult {
                                kind: Some(Kind::Verification(wire::VerificationTerminal {
                                    matched_enrollment: None,
                                    result: wire::VerificationResult::VerifyFactorAccepted as i32,
                                    error: None,
                                })),
                            }),
                        })),
                    },
                );
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    let clients: Vec<_> = (1..=2)
        .map(|id| {
            let path = path.clone();
            std::thread::spawn(move || {
                let request = wire::BeginVerification {
                    report_matched_enrollment: false,
                    enrollment: None,
                    idempotency_key: vec![id; 16],
                    subject: Some(common::SubjectSelector {
                        selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
                    }),
                    authenticator_id: vec![4; 16],
                    purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
                    authority_request_ref: vec![],
                };
                observe_on_new_connection(path.to_str().unwrap(), request)
            })
        })
        .collect();
    let mut results = clients.into_iter();
    assert_eq!(
        results.next().unwrap().join().unwrap().unwrap(),
        Observation::FactorAccepted
    );
    assert!(results.next().unwrap().join().unwrap().is_err());
    server.join().unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
