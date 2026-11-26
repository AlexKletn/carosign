use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{
    client::v1::{self as wire, client_frame::Payload},
    common::v1 as common,
};
use carosignctl::{client::ClientError, commands::Command};

use super::{
    authenticator_list::format_authenticators, execute, inventory::list_enrollments_connected,
    operation_id::parse_operation_id, operation_status::format_snapshot,
    verification::verify_connected,
};

#[test]
fn verify_uses_registered_device_and_reports_terminal_result() {
    use prost::Message;
    use std::os::fd::AsRawFd;
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let worker = std::thread::spawn(move || {
        for step in 0..4 {
            let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
            let count = unsafe {
                libc::recv(
                    pair.worker.as_raw_fd(),
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    0,
                )
            };
            assert!(count > 0);
            let request = wire::ClientFrame::decode(&bytes[..count as usize]).unwrap();
            let payload = match step {
                0 => {
                    assert!(matches!(request.payload, Some(Payload::Hello(_))));
                    Payload::HelloAck(wire::HelloAck {
                        verification_enrollment_selector: false,
                        capture_feedback_v1: false,
                        major: carosign_protocol::limits::MAJOR,
                        selected_minor: carosign_protocol::limits::MINOR_BASELINE,
                        connection_id: vec![3; 16],
                    })
                }
                1 => {
                    assert!(matches!(
                        request.payload,
                        Some(Payload::ListAuthenticators(_))
                    ));
                    Payload::Authenticators(wire::Authenticators {
                        providers: None,
                        entries: vec![common::AuthenticatorDescriptor {
                            authenticator_id: vec![1; 16],
                            available: true,
                            capabilities: Some(common::CapabilityDescriptor {
                                operations: vec![common::ProviderOperation::ProviderVerify as i32],
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                    })
                }
                2 => {
                    let Some(Payload::ListEnrollments(list)) = request.payload else {
                        panic!("неверный запрос")
                    };
                    assert_eq!(list.authenticator_id, vec![1; 16]);
                    assert_eq!(
                        list.subject.unwrap().selector,
                        Some(common::subject_selector::Selector::AccountHint(
                            "alex".into()
                        ))
                    );
                    Payload::Enrollments(wire::Enrollments {
                        entries: vec![common::EnrollmentSummary {
                            enrollment: Some(common::EnrollmentRef {
                                enrollment_id: vec![2; 16],
                                authenticator_id: vec![1; 16],
                                generation: 1,
                            }),
                            label: "палец".into(),
                            original_label: None,
                            pending_reconciliation: false,
                        }],
                    })
                }
                _ => {
                    let Some(Payload::BeginVerification(begin)) = request.payload else {
                        panic!("неверный запрос")
                    };
                    assert_eq!(begin.authenticator_id, vec![1; 16]);
                    assert_eq!(
                        begin.subject.unwrap().selector,
                        Some(common::subject_selector::Selector::AccountHint(
                            "alex".into()
                        ))
                    );
                    assert_eq!(
                        begin.purpose_hint,
                        common::VerificationPurpose::PurposeSelfCheck as i32
                    );
                    assert_eq!(begin.idempotency_key.len(), 16);
                    assert!(begin.idempotency_key.iter().any(|v| *v != 0));
                    Payload::OperationAccepted(wire::OperationAccepted {
                        operation_id: vec![4; 16],
                        first_event_sequence: 1,
                        deadline: Some(common::BootDeadline {
                            boot_id: vec![5; 16],
                            boottime_ns: 1,
                        }),
                    })
                }
            };
            let response = wire::ClientFrame {
                major: carosign_protocol::limits::MAJOR,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                request_id: request.request_id,
                operation_id: if step == 3 { vec![4; 16] } else { Vec::new() },
                event_sequence: 0,
                payload: Some(payload),
            }
            .encode_to_vec();
            assert_eq!(
                unsafe {
                    libc::send(
                        pair.worker.as_raw_fd(),
                        response.as_ptr().cast(),
                        response.len(),
                        libc::MSG_NOSIGNAL,
                    )
                },
                response.len() as isize
            );
        }
        let event = wire::ClientFrame {
            major: carosign_protocol::limits::MAJOR,
            minor: carosign_protocol::limits::MINOR_BASELINE,
            request_id: Vec::new(),
            operation_id: vec![4; 16],
            event_sequence: 1,
            payload: Some(Payload::OperationEvent(wire::OperationEvent {
                capture_state: None,
                feedback: None,
                operation_id: vec![4; 16],
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
                            result: wire::VerificationResult::VerifyNotMatched as i32,
                            error: None,
                        },
                    )),
                }),
            })),
        }
        .encode_to_vec();
        assert_eq!(
            unsafe {
                libc::send(
                    pair.worker.as_raw_fd(),
                    event.as_ptr().cast(),
                    event.len(),
                    libc::MSG_NOSIGNAL,
                )
            },
            event.len() as isize
        );
    });
    let transport = Transport::from_connected(pair.platform).unwrap();
    assert!(
        matches!(verify_connected(&transport, "alex"), Err(ClientError::Refused { message }) if message == carosign_l10n::Localizer::from_environment().text("verify-not-matched"))
    );
    worker.join().unwrap();
}

#[test]
fn lists_real_enrollments_through_client_protocol() {
    use prost::Message;
    use std::os::fd::AsRawFd;
    let dir = std::env::temp_dir().join(format!("carosignctl-enrollments-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("control.sock");
    let listener = carosign_linux::sys::listen_unix(path.to_str().unwrap()).unwrap();
    let worker = std::thread::spawn(move || {
        let (fd, _) = carosign_linux::sys::accept_connection(&listener).unwrap();
        for step in 0..3 {
            let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
            let count =
                unsafe { libc::recv(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
            assert!(count > 0);
            let request = wire::ClientFrame::decode(&bytes[..count as usize]).unwrap();
            let payload = match step {
                0 => {
                    assert!(matches!(request.payload, Some(Payload::Hello(_))));
                    Payload::HelloAck(wire::HelloAck {
                        verification_enrollment_selector: false,
                        capture_feedback_v1: false,
                        major: carosign_protocol::limits::MAJOR,
                        selected_minor: carosign_protocol::limits::MINOR_BASELINE,
                        connection_id: vec![3; 16],
                    })
                }
                1 => {
                    assert!(matches!(
                        request.payload,
                        Some(Payload::ListAuthenticators(_))
                    ));
                    Payload::Authenticators(wire::Authenticators {
                        providers: None,
                        entries: vec![common::AuthenticatorDescriptor {
                            authenticator_id: vec![1; 16],
                            available: false,
                            ..Default::default()
                        }],
                    })
                }
                _ => {
                    let Some(Payload::ListEnrollments(list)) = request.payload else {
                        panic!("неверный запрос")
                    };
                    assert_eq!(list.authenticator_id, vec![1; 16]);
                    assert_eq!(
                        list.subject.unwrap().selector,
                        Some(common::subject_selector::Selector::CurrentSubject(true))
                    );
                    Payload::Enrollments(wire::Enrollments {
                        entries: vec![common::EnrollmentSummary {
                            enrollment: Some(common::EnrollmentRef {
                                enrollment_id: vec![2; 16],
                                authenticator_id: vec![1; 16],
                                generation: 1,
                            }),
                            label: "палец".into(),
                            original_label: None,
                            pending_reconciliation: false,
                        }],
                    })
                }
            };
            let response = wire::ClientFrame {
                major: carosign_protocol::limits::MAJOR,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                request_id: request.request_id,
                operation_id: Vec::new(),
                event_sequence: 0,
                payload: Some(payload),
            }
            .encode_to_vec();
            assert_eq!(
                unsafe {
                    libc::send(
                        fd.as_raw_fd(),
                        response.as_ptr().cast(),
                        response.len(),
                        libc::MSG_NOSIGNAL,
                    )
                },
                response.len() as isize
            );
        }
    });
    let transport = Transport::connect(path.to_str().unwrap()).unwrap();
    let lines = list_enrollments_connected(&transport, None).unwrap();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("02020202020202020202020202020202"));
    assert!(lines[0].contains("палец"));
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn operation_status_requires_full_nonzero_id_and_consistent_snapshot() {
    let id = parse_operation_id("01010101010101010101010101010101").unwrap();
    assert!(parse_operation_id("01010101").is_err());
    assert!(parse_operation_id("00000000000000000000000000000000").is_err());
    assert!(parse_operation_id("é101010101010101010101010101010").is_err());
    let running = wire::OperationSnapshot {
        operation_id: id.to_vec(),
        phase: wire::OperationPhase::OpRunning as i32,
        last_event_sequence: 2,
        terminal: None,
    };
    assert!(format_snapshot(running.clone(), id).unwrap()[0].contains("выполняется"));
    assert!(format_snapshot(running.clone(), [2; 16]).is_err());
    let mut invalid = running;
    invalid.phase = wire::OperationPhase::OpTerminal as i32;
    assert!(format_snapshot(invalid, id).is_err());
}

#[test]
fn status_uses_get_operation_and_displays_snapshot() {
    use prost::Message;
    use std::os::fd::AsRawFd;

    let dir = std::env::temp_dir().join(format!("carosignctl-status-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("control.sock");
    let listener = carosign_linux::sys::listen_unix(path.to_str().unwrap()).unwrap();
    let worker = std::thread::spawn(move || {
        let (fd, _) = carosign_linux::sys::accept_connection(&listener).unwrap();
        let exchange = |expected_hello: bool| {
            let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
            let count =
                unsafe { libc::recv(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
            assert!(count > 0);
            let request = wire::ClientFrame::decode(&bytes[..count as usize]).unwrap();
            let payload = if expected_hello {
                assert!(matches!(request.payload, Some(Payload::Hello(_))));
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: carosign_protocol::limits::MAJOR,
                    selected_minor: carosign_protocol::limits::MINOR_BASELINE,
                    connection_id: vec![3; 16],
                })
            } else {
                let Some(Payload::GetOperation(get)) = request.payload else {
                    panic!("неверный запрос")
                };
                assert_eq!(get.operation_id, vec![1; 16]);
                Payload::OperationSnapshot(wire::OperationSnapshot {
                    operation_id: vec![1; 16],
                    phase: wire::OperationPhase::OpRunning as i32,
                    last_event_sequence: 2,
                    terminal: None,
                })
            };
            let response = wire::ClientFrame {
                major: carosign_protocol::limits::MAJOR,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                request_id: request.request_id,
                operation_id: Vec::new(),
                event_sequence: 0,
                payload: Some(payload),
            }
            .encode_to_vec();
            assert_eq!(
                unsafe {
                    libc::send(
                        fd.as_raw_fd(),
                        response.as_ptr().cast(),
                        response.len(),
                        libc::MSG_NOSIGNAL,
                    )
                },
                response.len() as isize
            );
        };
        exchange(true);
        exchange(false);
    });
    let command = Command::Status {
        operation: "01010101010101010101010101010101".into(),
    };
    assert!(execute(&command, path.to_str().unwrap()).unwrap()[0].contains("выполняется"));
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn cancel_requests_cancellation_and_reports_receipt() {
    use prost::Message;
    use std::os::fd::AsRawFd;

    let dir = std::env::temp_dir().join(format!("carosignctl-cancel-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("control.sock");
    let listener = carosign_linux::sys::listen_unix(path.to_str().unwrap()).unwrap();
    let worker = std::thread::spawn(move || {
        let (fd, _) = carosign_linux::sys::accept_connection(&listener).unwrap();
        for hello in [true, false] {
            let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
            let count =
                unsafe { libc::recv(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
            assert!(count > 0);
            let request = wire::ClientFrame::decode(&bytes[..count as usize]).unwrap();
            let payload = if hello {
                assert!(matches!(request.payload, Some(Payload::Hello(_))));
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: carosign_protocol::limits::MAJOR,
                    selected_minor: carosign_protocol::limits::MINOR_BASELINE,
                    connection_id: vec![3; 16],
                })
            } else {
                let Some(Payload::CancelOperation(cancel)) = request.payload else {
                    panic!("неверный запрос")
                };
                assert_eq!(cancel.operation_id, vec![1; 16]);
                Payload::CancellationReceipt(wire::CancellationReceipt {
                    result: wire::CancelReceiptKind::CancelReceiptRequested as i32,
                })
            };
            let response = wire::ClientFrame {
                major: carosign_protocol::limits::MAJOR,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                request_id: request.request_id,
                operation_id: Vec::new(),
                event_sequence: 0,
                payload: Some(payload),
            }
            .encode_to_vec();
            assert_eq!(
                unsafe {
                    libc::send(
                        fd.as_raw_fd(),
                        response.as_ptr().cast(),
                        response.len(),
                        libc::MSG_NOSIGNAL,
                    )
                },
                response.len() as isize
            );
        }
    });
    let command = Command::Cancel {
        operation: "01010101010101010101010101010101".into(),
    };
    assert_eq!(
        execute(&command, path.to_str().unwrap()).unwrap(),
        ["отмена запрошена"]
    );
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn authenticator_listing_rejects_oversized_and_invalid_ids() {
    use carosign_protocol::carosign::common::v1::AuthenticatorDescriptor;
    let valid = AuthenticatorDescriptor {
        authenticator_id: vec![1; 16],
        ..Default::default()
    };
    assert!(
        format_authenticators(wire::Authenticators {
            providers: None,
            entries: vec![valid.clone(); carosign_protocol::limits::DEVICES_PER_RESPONSE_MAX],
        })
        .is_ok()
    );
    assert!(matches!(
        format_authenticators(wire::Authenticators {
            providers: None,
            entries: vec![valid; carosign_protocol::limits::DEVICES_PER_RESPONSE_MAX + 1],
        }),
        Err(ClientError::Unexpected)
    ));
    for id in [vec![], vec![0; 16], vec![1; 15]] {
        assert!(matches!(
            format_authenticators(wire::Authenticators {
                providers: None,
                entries: vec![AuthenticatorDescriptor {
                    authenticator_id: id,
                    ..Default::default()
                }],
            }),
            Err(ClientError::Unexpected)
        ));
    }
}
