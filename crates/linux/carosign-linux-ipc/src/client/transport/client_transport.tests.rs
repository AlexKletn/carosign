use super::*;

#[test]
fn peer_uid_is_checked_by_kernel_credentials() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    // SAFETY: geteuid does not accept pointers and does not change state.
    let uid = unsafe { libc::geteuid() };
    assert!(require_peer_uid(&pair.platform, uid).is_ok());
    assert_eq!(
        require_peer_uid(&pair.platform, uid.wrapping_add(1)),
        Err(TransportError::Unavailable)
    );
}

struct SocketPath(std::path::PathBuf);

impl SocketPath {
    fn new() -> Self {
        let random = carosign_linux::sys::random_16().unwrap();
        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        Self(std::env::temp_dir().join(format!("bio-client-{suffix}.sock")))
    }
    fn as_str(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn connect_to_listener(path: &str) -> OwnedFd {
    // SAFETY: The descriptor, after a successful call, is received by the sole owner.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(raw >= 0);
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (dst, src) in address.sun_path.iter_mut().zip(path.as_bytes()) {
        *dst = *src as libc::c_char;
    }
    let len =
        (core::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + 1) as libc::socklen_t;
    // SAFETY: The address is filled in and available during the system call.
    assert_eq!(
        unsafe {
            libc::connect(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                len,
            )
        },
        0
    );
    fd
}

#[test]
fn full_listener_queue_cannot_block_client_connect() {
    let path = SocketPath::new();
    let listener = carosign_linux::sys::listen_unix(path.as_str()).unwrap();
    // SAFETY: We change the queue of only the test listener.
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
    let first = connect_to_listener(path.as_str());
    let (sender, receiver) = std::sync::mpsc::channel();
    let name = path.as_str().to_owned();
    let thread = std::thread::spawn(move || {
        let result = Transport::connect(&name);
        let _ = sender.send(result.is_err());
    });
    let result = receiver.recv_timeout(std::time::Duration::from_secs(2));
    // Closing the listener frees up the flow in case of failure of the check.
    drop(listener);
    drop(first);
    thread.join().unwrap();
    assert_eq!(result, Ok(true));
}

#[test]
fn connect_restores_blocking_mode_and_negotiates_on_real_listener() {
    let path = SocketPath::new();
    let listener = carosign_linux::sys::listen_unix(path.as_str()).unwrap();
    let server = std::thread::spawn(move || {
        let (fd, _) = carosign_linux::sys::accept_connection(&listener).unwrap();
        let hello = read(&fd);
        let id: [u8; 16] = hello.request_id.try_into().unwrap();
        send(
            &fd,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: limits::MAJOR,
                    selected_minor: limits::MINOR_BASELINE,
                    connection_id: vec![1; 16],
                }),
            ),
        )
        .unwrap();
    });
    let transport = Transport::connect(path.as_str()).unwrap();
    // SAFETY: Check the flags of the live descriptor belonging to the test.
    let flags = unsafe { libc::fcntl(transport.fd.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(flags & libc::O_NONBLOCK, 0);
    server.join().unwrap();
}

fn read(fd: &OwnedFd) -> wire::ClientFrame {
    let mut bytes = vec![0; limits::FRAME_MAX];
    // SAFETY: The buffer and descriptor are active during reading.
    let size = unsafe { libc::recv(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    assert!(size > 0);
    wire::ClientFrame::decode(&bytes[..size as usize]).unwrap()
}

fn peer(fd: OwnedFd, wrong_id: bool) {
    let hello = read(&fd);
    let id: [u8; 16] = hello.request_id.try_into().unwrap();
    send(
        &fd,
        &frame(
            &id,
            limits::MINOR_BASELINE,
            Payload::HelloAck(wire::HelloAck {
                verification_enrollment_selector: false,
                capture_feedback_v1: false,
                major: limits::MAJOR,
                selected_minor: limits::MINOR_BASELINE,
                connection_id: vec![1; 16],
            }),
        ),
    )
    .unwrap();
    let request = read(&fd);
    let id: [u8; 16] = if wrong_id {
        [7; 16]
    } else {
        request.request_id.try_into().unwrap()
    };
    send(
        &fd,
        &frame(
            &id,
            limits::MINOR_BASELINE,
            Payload::Health(wire::Health {
                service_ready: false,
                available_authenticators: 0,
                unavailable_authenticators: 2,
                scopes_needing_reconciliation: 1,
            }),
        ),
    )
    .unwrap();
}

#[test]
fn real_seqpacket_negotiates_and_checks_request_id() {
    for wrong in [false, true] {
        let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let thread = std::thread::spawn(move || peer(pair.worker, wrong));
        let transport = Transport::from_connected(pair.platform).unwrap();
        let result = transport.request(Payload::GetHealth(wire::GetHealth {}));
        if wrong {
            assert!(matches!(result, Err(TransportError::Unexpected)));
            assert!(matches!(
                transport.request(Payload::GetHealth(wire::GetHealth {})),
                Err(TransportError::Unavailable)
            ));
        } else {
            assert!(matches!(result.unwrap().payload, Some(Payload::Health(_))));
        }
        thread.join().unwrap();
    }
}

#[test]
fn verification_acceptance_binds_both_operation_ids_and_closes_on_mismatch() {
    use carosign_protocol::carosign::common::v1::{
        BootDeadline, SubjectSelector, subject_selector,
    };

    for mismatched in [false, true] {
        let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let peer = std::thread::spawn(move || {
            let hello = read(&pair.worker);
            let id: [u8; 16] = hello.request_id.try_into().unwrap();
            send(
                &pair.worker,
                &frame(
                    &id,
                    limits::MINOR_BASELINE,
                    Payload::HelloAck(wire::HelloAck {
                        verification_enrollment_selector: false,
                        capture_feedback_v1: false,
                        major: limits::MAJOR,
                        selected_minor: limits::MINOR_BASELINE,
                        connection_id: vec![1; 16],
                    }),
                ),
            )
            .unwrap();
            let request = read(&pair.worker);
            assert!(matches!(
                request.payload,
                Some(Payload::BeginVerification(_))
            ));
            let id: [u8; 16] = request.request_id.try_into().unwrap();
            let mut answer = frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::OperationAccepted(wire::OperationAccepted {
                    operation_id: vec![2; 16],
                    first_event_sequence: 1,
                    deadline: Some(BootDeadline {
                        boot_id: vec![3; 16],
                        boottime_ns: 1,
                    }),
                }),
            );
            answer.operation_id = vec![if mismatched { 4 } else { 2 }; 16];
            send(&pair.worker, &answer).unwrap();
        });
        let transport = Transport::from_connected(pair.platform).unwrap();
        let result = transport.request(Payload::BeginVerification(wire::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![5; 16],
            subject: Some(SubjectSelector {
                selector: Some(subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: vec![6; 16],
            purpose_hint: 1,
            authority_request_ref: Vec::new(),
        }));
        if mismatched {
            assert!(matches!(result, Err(TransportError::Unexpected)));
            assert!(matches!(
                transport.request(Payload::GetHealth(wire::GetHealth {})),
                Err(TransportError::Unavailable)
            ));
        } else {
            assert!(matches!(
                result.unwrap().payload,
                Some(Payload::OperationAccepted(_))
            ));
        }
        peer.join().unwrap();
    }
}

#[test]
fn mutation_acceptance_checks_operation_identity_on_real_socket() {
    use carosign_protocol::carosign::common::v1::{
        BootDeadline, EnrollmentRef, SubjectSelector, subject_selector,
    };

    for removal in [false, true] {
        for mismatched in [false, true] {
            let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
            let peer = std::thread::spawn(move || {
                let hello = read(&pair.worker);
                let id: [u8; 16] = hello.request_id.try_into().unwrap();
                send(
                    &pair.worker,
                    &frame(
                        &id,
                        limits::MINOR_BASELINE,
                        Payload::HelloAck(wire::HelloAck {
                            verification_enrollment_selector: false,
                            capture_feedback_v1: false,
                            major: limits::MAJOR,
                            selected_minor: limits::MINOR_BASELINE,
                            connection_id: vec![1; 16],
                        }),
                    ),
                )
                .unwrap();
                let request = read(&pair.worker);
                assert!(matches!(
                    request.payload,
                    Some(Payload::BeginEnrollment(_) | Payload::BeginRemoval(_))
                ));
                let id: [u8; 16] = request.request_id.try_into().unwrap();
                let mut answer = frame(
                    &id,
                    limits::MINOR_BASELINE,
                    Payload::OperationAccepted(wire::OperationAccepted {
                        operation_id: vec![2; 16],
                        first_event_sequence: 1,
                        deadline: Some(BootDeadline {
                            boot_id: vec![3; 16],
                            boottime_ns: 1,
                        }),
                    }),
                );
                answer.operation_id = vec![if mismatched { 4 } else { 2 }; 16];
                send(&pair.worker, &answer).unwrap();
            });
            let transport = Transport::from_connected(pair.platform).unwrap();
            let subject = Some(SubjectSelector {
                selector: Some(subject_selector::Selector::CurrentSubject(true)),
            });
            let payload = if removal {
                Payload::BeginRemoval(wire::BeginRemoval {
                    idempotency_key: vec![5; 16],
                    subject,
                    enrollment: Some(EnrollmentRef {
                        enrollment_id: vec![7; 16],
                        authenticator_id: vec![6; 16],
                        generation: 1,
                    }),
                    authority_request_ref: Vec::new(),
                })
            } else {
                Payload::BeginEnrollment(wire::BeginEnrollment {
                    idempotency_key: vec![5; 16],
                    subject,
                    authenticator_id: vec![6; 16],
                    label: "пример".into(),
                    replace: None,
                    authority_request_ref: Vec::new(),
                })
            };
            let result = transport.request(payload);
            if mismatched {
                assert!(matches!(result, Err(TransportError::Unexpected)));
                assert!(matches!(
                    transport.request(Payload::GetHealth(wire::GetHealth {})),
                    Err(TransportError::Unavailable)
                ));
            } else {
                assert!(matches!(
                    result.unwrap().payload,
                    Some(Payload::OperationAccepted(_))
                ));
            }
            peer.join().unwrap();
        }
    }
}

#[test]
fn verification_rejects_unrelated_response_and_closes_connection() {
    use carosign_protocol::carosign::common::v1::{SubjectSelector, subject_selector};

    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let thread = std::thread::spawn(move || peer(pair.worker, false));
    let transport = Transport::from_connected(pair.platform).unwrap();
    let result = transport.request(Payload::BeginVerification(wire::BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: vec![5; 16],
        subject: Some(SubjectSelector {
            selector: Some(subject_selector::Selector::CurrentSubject(true)),
        }),
        authenticator_id: vec![6; 16],
        purpose_hint: 1,
        authority_request_ref: Vec::new(),
    }));
    assert!(matches!(result, Err(TransportError::Unexpected)));
    assert!(matches!(
        transport.request(Payload::GetHealth(wire::GetHealth {})),
        Err(TransportError::Unavailable)
    ));
    thread.join().unwrap();
}

#[test]
fn verification_receives_event_on_real_socket_and_rejects_wrong_operation() {
    use carosign_protocol::carosign::client::v1::{EventKind, OperationEvent, OperationPhase};
    for wrong in [false, true] {
        let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
        let thread = std::thread::spawn(move || {
            let hello = read(&pair.worker);
            let id: [u8; 16] = hello.request_id.try_into().unwrap();
            send(
                &pair.worker,
                &frame(
                    &id,
                    limits::MINOR_BASELINE,
                    Payload::HelloAck(wire::HelloAck {
                        verification_enrollment_selector: false,
                        capture_feedback_v1: false,
                        major: limits::MAJOR,
                        selected_minor: limits::MINOR_BASELINE,
                        connection_id: vec![1; 16],
                    }),
                ),
            )
            .unwrap();
            let event = wire::ClientFrame {
                major: limits::MAJOR,
                minor: limits::MINOR_BASELINE,
                request_id: Vec::new(),
                operation_id: vec![if wrong { 3 } else { 2 }; 16],
                event_sequence: 1,
                payload: Some(Payload::OperationEvent(OperationEvent {
                    capture_state: None,
                    feedback: None,
                    operation_id: vec![2; 16],
                    sequence: 1,
                    kind: EventKind::EventWorking as i32,
                    phase: OperationPhase::OpRunning as i32,
                    percent: 0,
                    gap_after_sequence: 0,
                    snapshot: None,
                    terminal: None,
                })),
            };
            send(&pair.worker, &event).unwrap();
        });
        let transport = Transport::from_connected(pair.platform).unwrap();
        let mut tracker =
            crate::client::verification_events::VerificationEvents::new(&wire::OperationAccepted {
                operation_id: vec![2; 16],
                first_event_sequence: 1,
                deadline: None,
            })
            .unwrap();
        let result = transport.receive_verification_event(&mut tracker);
        if wrong {
            assert!(matches!(result, Err(TransportError::Unexpected)));
            assert!(matches!(
                transport.receive_verification_event(&mut tracker),
                Err(TransportError::Unavailable)
            ));
        } else {
            assert!(matches!(result, Ok((_, false))));
        }
        thread.join().unwrap();
    }
}

#[test]
fn cancellation_buffers_interleaved_event_for_its_owner() {
    use wire::{
        CancelOperation, CancelReceiptKind, CancellationReceipt, EventKind, OperationAccepted,
        OperationEvent, OperationPhase,
    };

    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = std::thread::spawn(move || {
        let hello = read(&pair.worker);
        let hello_id: [u8; 16] = hello.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &frame(
                &hello_id,
                limits::MINOR_BASELINE,
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: limits::MAJOR,
                    selected_minor: limits::MINOR_BASELINE,
                    connection_id: vec![1; 16],
                }),
            ),
        )
        .unwrap();
        let request = read(&pair.worker);
        assert!(matches!(request.payload, Some(Payload::CancelOperation(_))));
        let id: [u8; 16] = request.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &wire::ClientFrame {
                major: limits::MAJOR,
                minor: limits::MINOR_BASELINE,
                request_id: Vec::new(),
                operation_id: vec![2; 16],
                event_sequence: 1,
                payload: Some(Payload::OperationEvent(OperationEvent {
                    capture_state: None,
                    feedback: None,
                    operation_id: vec![2; 16],
                    sequence: 1,
                    kind: EventKind::EventWorking as i32,
                    phase: OperationPhase::OpRunning as i32,
                    percent: 0,
                    gap_after_sequence: 0,
                    snapshot: None,
                    terminal: None,
                })),
            },
        )
        .unwrap();
        send(
            &pair.worker,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::CancellationReceipt(CancellationReceipt {
                    result: CancelReceiptKind::CancelReceiptRequested as i32,
                }),
            ),
        )
        .unwrap();
    });
    let transport = Transport::from_connected(pair.platform).unwrap();
    let mut tracker =
        crate::client::verification_events::VerificationEvents::new(&OperationAccepted {
            operation_id: vec![2; 16],
            first_event_sequence: 1,
            deadline: None,
        })
        .unwrap();
    let response = transport
        .request(Payload::CancelOperation(CancelOperation {
            operation_id: vec![2; 16],
        }))
        .unwrap();
    assert!(matches!(
        response.payload,
        Some(Payload::CancellationReceipt(_))
    ));
    assert!(matches!(
        transport.receive_verification_event(&mut tracker),
        Ok((_, false))
    ));
    peer.join().unwrap();
}

#[test]
fn cancellation_can_finish_while_another_thread_waits_for_an_event() {
    use carosign_protocol::carosign::client::v1::{
        CancelOperation, CancelReceiptKind, CancellationReceipt, EventKind, OperationAccepted,
        OperationEvent, OperationPhase,
    };
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = std::thread::spawn(move || {
        let hello = read(&pair.worker);
        let id: [u8; 16] = hello.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: limits::MAJOR,
                    selected_minor: limits::MINOR_BASELINE,
                    connection_id: vec![1; 16],
                }),
            ),
        )
        .unwrap();
        let request = read(&pair.worker);
        assert!(matches!(request.payload, Some(Payload::CancelOperation(_))));
        let id: [u8; 16] = request.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::CancellationReceipt(CancellationReceipt {
                    result: CancelReceiptKind::CancelReceiptRequested as i32,
                }),
            ),
        )
        .unwrap();
        send(
            &pair.worker,
            &wire::ClientFrame {
                major: limits::MAJOR,
                minor: limits::MINOR_BASELINE,
                request_id: Vec::new(),
                operation_id: vec![2; 16],
                event_sequence: 1,
                payload: Some(Payload::OperationEvent(OperationEvent {
                    capture_state: None,
                    feedback: None,
                    operation_id: vec![2; 16],
                    sequence: 1,
                    kind: EventKind::EventWorking as i32,
                    phase: OperationPhase::OpRunning as i32,
                    percent: 0,
                    gap_after_sequence: 0,
                    snapshot: None,
                    terminal: None,
                })),
            },
        )
        .unwrap();
    });
    let transport = std::sync::Arc::new(Transport::from_connected(pair.platform).unwrap());
    let reader_transport = transport.clone();
    let waiting = std::sync::Arc::new(std::sync::Barrier::new(2));
    let reader_waiting = waiting.clone();
    let reader = std::thread::spawn(move || {
        let mut tracker =
            crate::client::verification_events::VerificationEvents::new(&OperationAccepted {
                operation_id: vec![2; 16],
                first_event_sequence: 1,
                deadline: None,
            })
            .unwrap();
        reader_waiting.wait();
        reader_transport.receive_verification_event(&mut tracker)
    });
    waiting.wait();
    let response = transport
        .request(Payload::CancelOperation(CancelOperation {
            operation_id: vec![2; 16],
        }))
        .unwrap();
    assert!(matches!(
        response.payload,
        Some(Payload::CancellationReceipt(_))
    ));
    assert!(matches!(reader.join().unwrap(), Ok((_, false))));
    peer.join().unwrap();
}

#[test]
fn concurrent_requests_do_not_consume_each_others_responses() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let peer_barrier = barrier.clone();
    let peer = std::thread::spawn(move || {
        let hello = read(&pair.worker);
        let id: [u8; 16] = hello.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: limits::MAJOR,
                    selected_minor: limits::MINOR_BASELINE,
                    connection_id: vec![1; 16],
                }),
            ),
        )
        .unwrap();
        peer_barrier.wait();
        for index in 0..2 {
            let request = read(&pair.worker);
            if index == 0 {
                let mut ready = libc::pollfd {
                    fd: pair.worker.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: pollfd is valid until the poll returns.
                assert_eq!(unsafe { libc::poll(&mut ready, 1, 150) }, 0);
            }
            let id: [u8; 16] = request.request_id.try_into().unwrap();
            send(
                &pair.worker,
                &frame(
                    &id,
                    limits::MINOR_BASELINE,
                    Payload::Health(wire::Health {
                        service_ready: false,
                        available_authenticators: 0,
                        unavailable_authenticators: 0,
                        scopes_needing_reconciliation: 0,
                    }),
                ),
            )
            .unwrap();
        }
    });
    let transport = std::sync::Arc::new(Transport::from_connected(pair.platform).unwrap());
    let clients: Vec<_> = (0..2)
        .map(|_| {
            let transport = transport.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                transport.request(Payload::GetHealth(wire::GetHealth {}))
            })
        })
        .collect();
    for client in clients {
        assert!(matches!(
            client.join().unwrap().unwrap().payload,
            Some(Payload::Health(_))
        ));
    }
    peer.join().unwrap();
}

#[test]
fn timed_out_response_cannot_be_reused_for_next_request() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = std::thread::spawn(move || {
        let hello = read(&pair.worker);
        let id: [u8; 16] = hello.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: limits::MAJOR,
                    selected_minor: limits::MINOR_BASELINE,
                    connection_id: vec![1; 16],
                }),
            ),
        )
        .unwrap();
        let request = read(&pair.worker);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let id: [u8; 16] = request.request_id.try_into().unwrap();
        assert!(
            send(
                &pair.worker,
                &frame(
                    &id,
                    limits::MINOR_BASELINE,
                    Payload::Health(wire::Health {
                        service_ready: false,
                        available_authenticators: 0,
                        unavailable_authenticators: 0,
                        scopes_needing_reconciliation: 0,
                    }),
                ),
            )
            .is_err()
        );
    });
    let transport = Transport::from_connected(pair.platform).unwrap();
    let timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: 50_000,
    };
    // SAFETY: We cut the timeout only on the socket belonging to the test.
    assert_eq!(
        unsafe {
            libc::setsockopt(
                transport.fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&timeout as *const libc::timeval).cast(),
                core::mem::size_of_val(&timeout) as libc::socklen_t,
            )
        },
        0
    );
    assert!(matches!(
        transport.request(Payload::GetHealth(wire::GetHealth {})),
        Err(TransportError::Unavailable)
    ));
    assert!(matches!(
        transport.request(Payload::GetHealth(wire::GetHealth {})),
        Err(TransportError::Unavailable)
    ));
    peer.join().unwrap();
}

#[test]
fn duplicate_payload_is_refused() {
    let mut bytes = frame(
        &[1; 16],
        limits::MINOR_BASELINE,
        Payload::GetHealth(wire::GetHealth {}),
    )
    .encode_to_vec();
    bytes.extend_from_slice(
        &frame(
            &[2; 16],
            limits::MINOR_BASELINE,
            Payload::GetHealth(wire::GetHealth {}),
        )
        .encode_to_vec(),
    );
    assert_eq!(check_one_payload(&bytes), Err(TransportError::Unexpected));
}

#[test]
fn oversized_packet_is_refused() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let bytes = vec![1u8; limits::FRAME_MAX + 1];
    // SAFETY: transmit a real buffer and a live descriptor.
    let sent = unsafe {
        libc::send(
            pair.worker.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            0,
        )
    };
    assert_eq!(sent, bytes.len() as isize);
    assert!(matches!(
        receive(&pair.platform, &[1; 16]),
        Err(TransportError::Unexpected)
    ));
}

#[test]
fn incompatible_hello_is_refused() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let thread = std::thread::spawn(move || {
        let hello = read(&pair.worker);
        let id: [u8; 16] = hello.request_id.try_into().unwrap();
        send(
            &pair.worker,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::HelloAck(wire::HelloAck {
                    verification_enrollment_selector: false,
                    capture_feedback_v1: false,
                    major: limits::MAJOR + 1,
                    selected_minor: limits::MINOR_BASELINE,
                    connection_id: vec![1; 16],
                }),
            ),
        )
        .unwrap();
    });
    assert!(matches!(
        Transport::from_connected(pair.platform),
        Err(TransportError::Unexpected)
    ));
    thread.join().unwrap();
}

#[test]
fn ancillary_file_descriptor_is_refused() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let packet = frame(
        &[1; 16],
        limits::MINOR_BASELINE,
        Payload::GetHealth(wire::GetHealth {}),
    )
    .encode_to_vec();
    let mut iov = libc::iovec {
        iov_base: packet.as_ptr().cast_mut().cast(),
        iov_len: packet.len(),
    };
    // Alignment is required for cmsghdr; one FD is transmitted to the kernel.
    let mut control = [0usize; 8];
    let mut message: libc::msghdr = unsafe { core::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen =
        unsafe { libc::CMSG_SPACE(core::mem::size_of::<libc::c_int>() as u32) } as usize;
    // SAFETY: Control is aligned and large enough for a single descriptor.
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        assert!(!header.is_null());
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(core::mem::size_of::<libc::c_int>() as u32) as usize;
        libc::CMSG_DATA(header)
            .cast::<libc::c_int>()
            .write(pair.worker.as_raw_fd());
        assert_eq!(
            libc::sendmsg(pair.worker.as_raw_fd(), &message, 0),
            packet.len() as isize
        );
    }
    assert!(matches!(
        receive(&pair.platform, &[1; 16]),
        Err(TransportError::Unexpected)
    ));
}

#[test]
fn stalled_peer_has_bounded_wait() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let start = std::time::Instant::now();
    let result = Transport::from_connected(pair.platform);
    assert!(matches!(result, Err(TransportError::Unavailable)));
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
    drop(pair.worker);
}

#[test]
fn stream_socket_is_refused() {
    let (stream, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    assert!(matches!(
        Transport::from_connected(stream.into()),
        Err(TransportError::Unavailable)
    ));
}
