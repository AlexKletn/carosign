use super::*;

struct FailedConnectionIds;
macro_rules! unused_id {
    ($name:ident, $kind:ident) => {
        fn $name(&self) -> Result<carosign_core::ids::$kind, carosign_core::ids::IdUnavailable> {
            Err(carosign_core::ids::IdUnavailable)
        }
    };
}
impl IdSource for FailedConnectionIds {
    unused_id!(new_operation_id, OperationId);
    unused_id!(new_mutation_id, MutationId);
    unused_id!(new_command_id, CommandId);
    unused_id!(new_worker_instance_id, WorkerInstanceId);
    unused_id!(new_lease_id, LeaseId);
    unused_id!(new_evidence_id, EvidenceId);
    unused_id!(new_idempotency_key, IdempotencyKey);
    unused_id!(new_connection_id, ConnectionId);
}

#[test]
fn connection_id_refuses_unavailable_or_repeated_zero_source() {
    assert_eq!(
        fresh_connection_id(|| Err(sys::SysError {
            errno: libc::EAGAIN
        })),
        Err(sys::SysError {
            errno: libc::EAGAIN
        })
    );
    let mut calls = 0;
    assert_eq!(
        fresh_connection_id(|| {
            calls += 1;
            Ok([0; 16])
        }),
        Err(sys::SysError { errno: libc::EIO })
    );
    assert_eq!(calls, 8);
    let mut calls = 0;
    assert_eq!(
        fresh_connection_id(|| {
            calls += 1;
            Ok(if calls == 1 { [0; 16] } else { [3; 16] })
        }),
        Ok([3; 16])
    );
}

#[test]
fn failed_random_source_closes_real_accepted_connection() {
    let path = std::env::temp_dir().join(format!(
        "carosign-random-refusal-{}.sock",
        std::process::id()
    ));
    let path = path.to_str().unwrap().to_owned();
    let listener = sys::listen_unix(&path).unwrap();
    let client_path = path.clone();
    let client = std::thread::spawn(move || {
        carosign_linux_ipc::client::transport::Transport::connect(&client_path).is_err()
    });
    let registry = crate::operations::OperationRegistry::new();
    assert!(matches!(
        accept_counted(&listener, &registry, 4, &FailedConnectionIds),
        Err(CountedAcceptError::RandomUnavailable)
    ));
    assert!(client.join().unwrap());
    let client_path = path.clone();
    let next_client = std::thread::spawn(move || {
        carosign_linux_ipc::client::transport::Transport::connect(&client_path).is_err()
    });
    let next = accept_counted(&listener, &registry, 4, &KernelIds::new())
        .unwrap_or_else(|_| panic!("следующее соединение не принято"));
    drop(next);
    assert!(next_client.join().unwrap());
    let handle = registry.open_connection(4).expect("место не занято");
    registry.close_connection(handle);
    drop(listener);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn counted_connection_releases_its_bounded_slot_on_drop() {
    let registry = crate::operations::OperationRegistry::new();
    let counted = register_counted(connection(), &registry, 4).ok().unwrap();
    for _ in 1..crate::operations::DEFAULT_CONNECTIONS {
        assert!(registry.open_connection(4).is_some());
    }
    assert!(registry.open_connection(4).is_none());
    drop(counted);
    assert!(registry.open_connection(4).is_some());
}

fn connection() -> Connection {
    let pair = sys::socketpair_seqpacket().expect("пара");
    let credentials = PeerCredentials::from_kernel(std::process::id(), 1000, 1000).expect("креды");
    Connection {
        session: ClientSession::new(credentials, [1u8; 16]),
        peer: PeerUser {
            pid: std::process::id(),
            uid: 1000,
            gid: 1000,
        },
        fd: pair.platform,
    }
}

#[test]
fn slow_peer_returns_backpressure_instead_of_blocking_sender() {
    let pair = sys::socketpair_seqpacket().unwrap();
    let peer = sys::peer_credentials(&pair.platform).unwrap();
    let credentials = PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid).unwrap();
    let connection = Connection {
        session: ClientSession::new(credentials, [1; 16]),
        peer,
        fd: pair.platform,
    };
    let bytes = [7u8; 4096];
    let mut full = false;
    for _ in 0..10_000 {
        match write_frame(&connection, &bytes) {
            Ok(()) => {}
            Err(error) if error == libc::EAGAIN => {
                full = true;
                break;
            }
            Err(error) => panic!("неожиданный отказ записи: {error}"),
        }
    }
    assert!(full, "очередь не достигла предела");
    assert_eq!(write_frame(&connection, &[]), Err(libc::EMSGSIZE));
    drop(pair.worker);
}

fn request(payload: Payload) -> Vec<u8> {
    ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: vec![1u8; 16],
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(payload),
    }
    .encode_to_vec()
}

#[test]
fn hello_gets_an_ack() {
    for enabled in [false, true] {
        let mut connection = connection();
        let hello = request(Payload::Hello(client::Hello {
            capture_feedback_v1: enabled,
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
        }));
        let answer = greet(&mut connection, &hello).unwrap();
        let frame = ClientFrame::decode(answer.as_slice()).unwrap();
        let Some(Payload::HelloAck(ack)) = frame.payload else {
            panic!("expected HelloAck");
        };
        assert_eq!(ack.selected_minor, carosign_protocol::limits::MINOR_MAX);
        assert_eq!(ack.capture_feedback_v1, enabled);
        assert_eq!(connection.session.capture_feedback_v1(), enabled);
    }
}

#[test]
fn anything_else_is_refused() {
    let mut connection = connection();
    let health = request(Payload::GetHealth(client::GetHealth {}));
    assert!(matches!(
        greet(&mut connection, &health),
        Err(ConversationEnd::Refused(_))
    ));
    // And after the rejection, the session is not "agreed."
    assert_eq!(
        connection.session.phase(),
        carosign_linux_ipc::client::session::SessionPhase::AwaitingHello
    );
}

#[test]
fn bad_version_is_refused_before_the_ack() {
    let mut connection = connection();
    let mut hello = ClientFrame::decode(
        &request(Payload::Hello(client::Hello {
            capture_feedback_v1: false,
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
        }))[..],
    )
    .expect("разбор");
    hello.major = 2;
    assert!(matches!(
        greet(&mut connection, &hello.encode_to_vec()),
        Err(ConversationEnd::Refused(_))
    ));
}

#[test]
fn connection_ids_do_not_repeat() {
    let a = new_connection().unwrap();
    let b = new_connection().unwrap();
    assert_ne!(a, b);
    assert!(a.iter().any(|byte| *byte != 0));
}

#[test]
fn public_socket_rejects_truncated_packets_and_passed_fds() {
    use std::os::fd::AsRawFd;
    let pair = sys::socketpair_seqpacket().unwrap();
    let credentials = PeerCredentials::from_kernel(std::process::id(), 1000, 1000).unwrap();
    let connection = Connection {
        session: ClientSession::new(credentials, [1; 16]),
        peer: PeerUser {
            pid: std::process::id(),
            uid: 1000,
            gid: 1000,
        },
        fd: pair.platform,
    };
    let payload = [7u8; 16];
    // SAFETY: The descriptor and buffer are valid at the time of the call.
    let sent = unsafe {
        libc::send(
            pair.worker.as_raw_fd(),
            payload.as_ptr().cast(),
            payload.len(),
            0,
        )
    };
    assert_eq!(sent, payload.len() as isize);
    assert_eq!(read_frame(&connection, &mut [0u8; 8]), Err(libc::EMSGSIZE));
    sys::send_fd(&pair.worker, &pair.worker).unwrap();
    assert_eq!(read_frame(&connection, &mut [0u8; 8]), Err(libc::EMSGSIZE));
}
