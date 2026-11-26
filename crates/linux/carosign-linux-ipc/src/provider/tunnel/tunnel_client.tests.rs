use super::super::{HelloError, TunnelProviderChannel};
use carosign_protocol::carosign::broker_relay::v1::{Control, control};

use super::*;

#[test]
fn broker_send_boundary_excludes_lease_change_without_locking_reply() {
    use carosign_protocol::carosign::broker_relay::v1::CommandResult;
    use std::sync::{Arc, Mutex, mpsc};
    let (fd, peer) = socket_pair();
    let mut client = TunnelClient {
        fd,
        encoder: TunnelCommandEncoder::after_hello([1; 16], 4, [3; 16]).unwrap(),
        worker: [1; 16],
        epoch: 4,
        next_in: 2,
        provider_in: carosign_provider_sdk::wire::Channel::new(),
        pending_events: VecDeque::new(),
        pending_authenticator: None,
        pending_prebind: None,
        pending_challenge: None,
        capture_feedback_enabled: false,
        capture_device: None,
        closed: false,
    };
    let (arrived_tx, arrived_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    let broker = std::thread::spawn(move || {
        let request = Frame::decode(receive(&peer).as_slice()).unwrap();
        let Some(Body::Command(command)) = request.body else {
            panic!("command")
        };
        arrived_tx.send(()).unwrap();
        reply_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        send(
            &peer,
            &Frame {
                major: 1,
                minor: 0,
                seq: 2,
                worker_instance: vec![1; 16],
                device_epoch: 4,
                body: Some(Body::CommandResult(CommandResult {
                    request_id: command.request_id,
                    result: Some(command_result::Result::Sent(true)),
                })),
            }
            .encode_to_vec(),
        );
    });
    let live = Arc::new(Mutex::new(true));
    let held = live.clone();
    let sender = std::thread::spawn(move || {
        let frame = ProviderFrame {
            worker_instance_id: vec![1; 16],
            device_epoch: 4,
            payload: Some(Payload::Describe(Default::default())),
            ..Default::default()
        };
        client.send_command_with_boundary(frame, None, Duration::from_secs(3), &mut |send| {
            let lease = held.lock().unwrap();
            *lease && send()
        })
    });
    arrived_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    *live.lock().unwrap() = false;
    reply_tx.send(()).unwrap();
    assert!(sender.join().unwrap().is_ok());
    broker.join().unwrap();

    let (fd, peer) = socket_pair();
    let mut client = TunnelClient {
        fd,
        encoder: TunnelCommandEncoder::after_hello([1; 16], 4, [3; 16]).unwrap(),
        worker: [1; 16],
        epoch: 4,
        next_in: 2,
        provider_in: carosign_provider_sdk::wire::Channel::new(),
        pending_events: VecDeque::new(),
        pending_authenticator: None,
        pending_prebind: None,
        pending_challenge: None,
        capture_feedback_enabled: false,
        capture_device: None,
        closed: false,
    };
    let frame = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        payload: Some(Payload::Describe(Default::default())),
        ..Default::default()
    };
    assert!(
        client
            .send_command_with_boundary(frame, None, Duration::from_secs(2), &mut |send| {
                let lease = live.lock().unwrap();
                *lease && send()
            })
            .is_err()
    );
    let mut buf = [0u8; 1];
    assert_eq!(
        unsafe { libc::recv(peer.as_raw_fd(), buf.as_mut_ptr().cast(), 1, 0) },
        0
    );
}

#[test]
fn challenge_reply_must_match_pending_command_operation_and_mutation() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::provider::v1::EnrollmentChallengeReady;
    for wrong in 0..4 {
        let (fd, _peer) = socket_pair();
        let mut client = TunnelClient {
            fd,
            encoder: TunnelCommandEncoder::after_hello([1; 16], 4, [3; 16]).unwrap(),
            worker: [1; 16],
            epoch: 4,
            next_in: 2,
            provider_in: carosign_provider_sdk::wire::Channel::new(),
            pending_events: VecDeque::new(),
            pending_authenticator: None,
            pending_prebind: None,
            pending_challenge: Some((vec![5; 16], vec![6; 16], vec![7; 16])),
            capture_feedback_enabled: false,
            capture_device: None,
            closed: false,
        };
        let frame = ProviderFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            worker_instance_id: vec![1; 16],
            frame_seq: 1,
            device_epoch: 4,
            command_id: vec![if wrong == 1 { 8 } else { 5 }; 16],
            operation_id: vec![if wrong == 2 { 8 } else { 6 }; 16],
            uid: None,
            payload: Some(Payload::EnrollmentChallengeReady(
                EnrollmentChallengeReady {
                    mutation_id: vec![if wrong == 3 { 8 } else { 7 }; 16],
                    challenge: vec![9],
                    challenge_ref: vec![10; 16],
                    validity_ms: 10,
                },
            )),
        };
        let relay = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 4,
            body: Some(Body::Event(Event { frame: Some(frame) })),
        }
        .encode_to_vec();
        let result = client.parse_event(&relay);
        assert_eq!(result.is_ok(), wrong == 0);
        assert!(client.pending_challenge.is_none());
    }
}

#[test]
fn challenge_wire_rejects_duplicate_fields() {
    // Raw answer with two meanings validity_ms It can't be reduced to the last.
    let provider = [0xa2, 0x02, 0x04, 0x20, 0x01, 0x20, 0x02];
    let mut event = vec![0x0a, provider.len() as u8];
    event.extend_from_slice(&provider);
    let mut relay = vec![0x62, event.len() as u8];
    relay.extend_from_slice(&event);
    assert!(strict_event(&relay).is_err());
}
use std::os::fd::{AsRawFd, FromRawFd};

fn socket_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    // SAFETY: Socketpair fills two elements of the array.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) },
        0
    );
    // SAFETY: After a successful call, both descriptors belong to the test.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn receive(fd: &OwnedFd) -> Vec<u8> {
    let mut data = vec![0u8; carosign_protocol::limits::FRAME_MAX];
    // SAFETY: The buffer and descriptor remain valid throughout the call.
    let n = unsafe { libc::recv(fd.as_raw_fd(), data.as_mut_ptr().cast(), data.len(), 0) };
    assert!(n > 0);
    data.truncate(n as usize);
    data
}

fn send(fd: &OwnedFd, data: &[u8]) {
    // SAFETY: The buffer and descriptor remain valid throughout the call.
    assert_eq!(
        unsafe {
            libc::send(
                fd.as_raw_fd(),
                data.as_ptr().cast(),
                data.len(),
                libc::MSG_NOSIGNAL,
            )
        },
        data.len() as isize
    );
}

#[test]
fn cancel_receipt_rejects_duplicate_status() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::provider::v1::{CancelReceipt, CancelStatus};

    let provider = ProviderFrame {
        major: 1,
        minor: 3,
        frame_seq: 2,
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        operation_id: vec![4; 16],
        command_id: vec![5; 16],
        uid: None,
        payload: Some(Payload::CancelReceipt(CancelReceipt {
            status: CancelStatus::CancelRequested as i32,
        })),
    };
    let relay = Frame {
        major: 1,
        minor: 0,
        seq: 3,
        worker_instance: vec![1; 16],
        device_epoch: 4,
        body: Some(Body::Event(Event {
            frame: Some(provider),
        })),
    }
    .encode_to_vec();
    assert!(strict_event(&relay).is_ok());
    // Repeat status is rejected until prost-decoding.
    let provider = [0x9a, 0x02, 0x04, 0x08, 0x01, 0x08, 0x01];
    let mut event = vec![0x0a, provider.len() as u8];
    event.extend_from_slice(&provider);
    let mut relay = vec![0x62, event.len() as u8];
    relay.extend_from_slice(&event);
    assert!(strict_event(&relay).is_err());
}

#[test]
fn command_ack_rejects_duplicate_nested_error_fields() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::provider::v1::CommandAck;

    let provider = ProviderFrame {
        major: 1,
        minor: 3,
        frame_seq: 2,
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        operation_id: vec![4; 16],
        command_id: vec![5; 16],
        uid: None,
        payload: Some(Payload::CommandAck(CommandAck {
            accepted: true,
            rejection: None,
        })),
    };
    let relay = Frame {
        major: 1,
        minor: 0,
        seq: 3,
        worker_instance: vec![1; 16],
        device_epoch: 4,
        body: Some(Body::Event(Event {
            frame: Some(provider),
        })),
    }
    .encode_to_vec();
    assert!(strict_event(&relay).is_ok());

    // Repeat the field in the nested Error must be deflected before decoding the prost.
    let provider = [0xf2, 0x01, 0x06, 0x12, 0x04, 0x08, 0x01, 0x08, 0x01];
    let mut event = vec![0x0a, provider.len() as u8];
    event.extend_from_slice(&provider);
    let mut relay = vec![0x62, event.len() as u8];
    relay.extend_from_slice(&event);
    assert!(strict_event(&relay).is_err());
}

#[test]
fn verification_outcome_rejects_ambiguous_evidence() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::provider::v1::{VerificationOutcome, VerificationStatus};
    let provider = ProviderFrame {
        major: 1,
        minor: 3,
        frame_seq: 2,
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        operation_id: vec![4; 16],
        command_id: vec![5; 16],
        uid: None,
        payload: Some(Payload::VerificationOutcome(VerificationOutcome {
            status: VerificationStatus::VerifyNotMatched as i32,
            ..Default::default()
        })),
    };
    let relay = Frame {
        major: 1,
        minor: 0,
        seq: 3,
        worker_instance: vec![1; 16],
        device_epoch: 4,
        body: Some(Body::Event(Event {
            frame: Some(provider),
        })),
    }
    .encode_to_vec();
    assert!(strict_event(&relay).is_ok());
    // At the same time, both branches of oneof should not be treated as the last.
    let ambiguous = [0x12, 0x01, 0x01, 0x1a, 0x01, 0x01];
    let mut raw_provider = vec![
        0x08,
        0x01,
        0x10,
        0x03,
        0x20,
        0x02,
        0x82,
        0x02,
        ambiguous.len() as u8,
    ];
    raw_provider.extend_from_slice(&ambiguous);
    let mut raw_event = vec![0x0a, raw_provider.len() as u8];
    raw_event.extend_from_slice(&raw_provider);
    let mut raw_relay = vec![0x08, 0x01, 0x18, 0x03, 0x62, raw_event.len() as u8];
    raw_relay.extend_from_slice(&raw_event);
    assert!(strict_event(&raw_relay).is_err());
}

#[test]
fn initialized_event_accepts_only_its_wire_schema() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::provider::v1::Initialized;
    let provider = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 2,
        device_epoch: 0,
        payload: Some(Payload::Initialized(Initialized {
            ready_for_binding: true,
        })),
        ..Default::default()
    };
    let relay = Frame {
        major: 1,
        minor: 0,
        seq: 3,
        worker_instance: vec![1; 16],
        device_epoch: 4,
        body: Some(Body::Event(Event {
            frame: Some(provider),
        })),
    }
    .encode_to_vec();
    assert!(strict_event(&relay).is_ok());

    // Twice the field of the result cannot be reduced to the last.
    assert!(strict_event(&[0x62, 0x08, 0x0a, 0x06, 0x62, 0x04, 0x08, 0x01, 0x08, 0x01]).is_err());
}

#[test]
fn event_before_command_reply_is_retained() {
    use carosign_protocol::carosign::broker_relay::v1::{CommandResult, Event};
    use carosign_protocol::carosign::provider::v1::{Describe, WorkerHello};
    let (fd, peer) = socket_pair();
    let child = std::thread::spawn(move || {
        send(&peer, &receive(&peer));
        let request = Frame::decode(receive(&peer).as_slice()).unwrap();
        let Some(Body::Command(command)) = request.body else {
            panic!()
        };
        let event = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 4,
            body: Some(Body::Event(Event {
                frame: Some(ProviderFrame {
                    major: 1,
                    minor: 3,
                    frame_seq: 1,
                    payload: Some(Payload::WorkerHello(WorkerHello {
                        major: 1,
                        min_minor: 3,
                        max_minor: 3,
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
            })),
        };
        send(&peer, &event.encode_to_vec());
        let ack = Frame {
            major: 1,
            minor: 0,
            seq: 3,
            worker_instance: vec![1; 16],
            device_epoch: 4,
            body: Some(Body::Event(Event {
                frame: Some(ProviderFrame {
                    major: 1,
                    minor: 3,
                    worker_instance_id: vec![1; 16],
                    device_epoch: 4,
                    frame_seq: 2,
                    command_id: vec![5; 16],
                    payload: Some(Payload::CommandAck(
                        carosign_protocol::carosign::provider::v1::CommandAck {
                            accepted: true,
                            rejection: None,
                        },
                    )),
                    ..Default::default()
                }),
            })),
        };
        send(&peer, &ack.encode_to_vec());
        let answer = Frame {
            major: 1,
            minor: 0,
            seq: 4,
            worker_instance: vec![1; 16],
            device_epoch: 4,
            body: Some(Body::CommandResult(CommandResult {
                request_id: command.request_id,
                result: Some(command_result::Result::Sent(true)),
            })),
        };
        send(&peer, &answer.encode_to_vec());
    });
    let mut client =
        TunnelClient::connect(fd, [1; 16], 4, [2; 16], [3; 16], Duration::from_secs(2)).unwrap();
    client
        .command(
            ProviderFrame {
                worker_instance_id: vec![1; 16],
                device_epoch: 4,
                payload: Some(Payload::Describe(Describe::default())),
                ..Default::default()
            },
            Duration::from_secs(2),
        )
        .unwrap();
    assert!(matches!(
        client.receive_event(Duration::ZERO).unwrap().payload,
        Some(Payload::WorkerHello(_))
    ));
    assert!(matches!(
        client.receive_event(Duration::ZERO).unwrap().payload,
        Some(Payload::CommandAck(_))
    ));
    child.join().unwrap();
}

#[test]
fn separate_broker_events_are_checked_before_decode() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::provider::v1::{ProgressKind, ProviderProgress, WorkerHello};
    use std::os::unix::process::CommandExt;
    const CHILD_ENV: &str = "CAROSIGN_TUNNEL_EVENTS_CHILD_FD";
    if let Ok(raw) = std::env::var(CHILD_ENV) {
        // SAFETY: The selected descriptor is transmitted by a parental test.
        let peer = unsafe { OwnedFd::from_raw_fd(raw.parse().unwrap()) };
        send(&peer, &receive(&peer));
        let wrap = |seq, frame| {
            Frame {
                major: 1,
                minor: 0,
                seq,
                worker_instance: vec![1; 16],
                device_epoch: 4,
                body: Some(Body::Event(Event { frame: Some(frame) })),
            }
            .encode_to_vec()
        };
        send(
            &peer,
            &wrap(
                2,
                ProviderFrame {
                    major: 1,
                    minor: 3,
                    frame_seq: 1,
                    payload: Some(Payload::WorkerHello(WorkerHello {
                        major: 1,
                        min_minor: 3,
                        max_minor: 3,
                        ..Default::default()
                    })),
                    ..Default::default()
                },
            ),
        );
        send(
            &peer,
            &wrap(
                3,
                ProviderFrame {
                    major: 1,
                    minor: 3,
                    frame_seq: 2,
                    worker_instance_id: vec![1; 16],
                    device_epoch: 4,
                    operation_id: vec![4; 16],
                    command_id: vec![5; 16],
                    uid: None,
                    payload: Some(Payload::ProviderProgress(ProviderProgress {
                        capture_state: None,
                        feedback: None,
                        operation_sequence: 1,
                        kind: ProgressKind::ProgressWorking as i32,
                        percent: 0,
                    })),
                },
            ),
        );
        let mut bad = wrap(
            4,
            ProviderFrame {
                major: 1,
                minor: 3,
                frame_seq: 3,
                worker_instance_id: vec![1; 16],
                device_epoch: 4,
                operation_id: vec![4; 16],
                command_id: vec![5; 16],
                uid: None,
                payload: Some(Payload::ProviderProgress(ProviderProgress {
                    capture_state: None,
                    feedback: None,
                    operation_sequence: 2,
                    kind: ProgressKind::ProgressWorking as i32,
                    percent: 0,
                })),
            },
        );
        // Repeat the singular field of the outer envelope should not be lost in the prost.
        bad.extend([0x18, 0x04]);
        send(&peer, &bad);
        return;
    }
    let (fd, peer) = socket_pair();
    let peer_fd = peer.as_raw_fd();
    const CHILD_FD: i32 = 198;
    let mut child_cmd = std::process::Command::new(std::env::current_exe().unwrap());
    child_cmd
        .args([
            "--exact",
            "provider::tunnel::tunnel_client::tests::separate_broker_events_are_checked_before_decode",
            "--nocapture",
        ])
        .env(CHILD_ENV, CHILD_FD.to_string());
    // SAFETY: After fork, only dup2 is used.
    unsafe {
        child_cmd.pre_exec(move || {
            if libc::dup2(peer_fd, CHILD_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = child_cmd.spawn().unwrap();
    drop(peer);
    let mut client =
        TunnelClient::connect(fd, [1; 16], 4, [2; 16], [3; 16], Duration::from_secs(5)).unwrap();
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(5))
            .unwrap()
            .payload,
        Some(Payload::WorkerHello(_))
    ));
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(5))
            .unwrap()
            .payload,
        Some(Payload::ProviderProgress(_))
    ));
    assert!(matches!(
        client.receive_event(Duration::from_secs(5)),
        Err(TunnelError::Invalid)
    ));
    assert!(matches!(
        client.receive_event(Duration::from_secs(5)),
        Err(TunnelError::Closed)
    ));
    assert!(child.wait().unwrap().success());
}

#[test]
fn separate_broker_exit_after_command_closes_channel_without_retry() {
    use carosign_protocol::carosign::provider::v1::Describe;
    use std::os::unix::process::CommandExt;

    const CHILD_ENV: &str = "CAROSIGN_TUNNEL_EXIT_AFTER_COMMAND_FD";
    if let Ok(raw) = std::env::var(CHILD_ENV) {
        // SAFETY: Only the test parent transmits the allocated FD.
        let peer = unsafe { OwnedFd::from_raw_fd(raw.parse().unwrap()) };
        let hello = receive(&peer);
        send(&peer, &hello);
        let command = Frame::decode(receive(&peer).as_slice()).unwrap();
        assert!(matches!(command.body, Some(Body::Command(_))));
        // The connection closes without a result; the shipment may have taken place.
        return;
    }

    let (fd, peer) = socket_pair();
    let peer_fd = peer.as_raw_fd();
    const CHILD_FD: i32 = 198;
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
            .args(["--exact", "provider::tunnel::tunnel_client::tests::separate_broker_exit_after_command_closes_channel_without_retry", "--nocapture"])
            .env(CHILD_ENV, CHILD_FD.to_string());
    // SAFETY: Only async-signal-safe dup2 is called after fork and before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(peer_fd, CHILD_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    drop(peer);
    let mut client =
        TunnelClient::connect(fd, [1; 16], 4, [2; 16], [3; 16], Duration::from_secs(5)).unwrap();
    let frame = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        payload: Some(Payload::Describe(Describe::default())),
        ..Default::default()
    };
    assert!(
        client
            .command(frame.clone(), Duration::from_secs(5))
            .is_err()
    );
    assert!(matches!(
        client.command(frame, Duration::from_secs(5)),
        Err(TunnelError::Closed)
    ));
    assert!(child.wait().unwrap().success());
}

#[test]
fn socket_command_checks_reply_and_closes_after_ambiguous_result() {
    use carosign_protocol::carosign::broker_relay::v1::CommandResult;
    use carosign_protocol::carosign::provider::v1::Describe;
    let (fd, peer) = socket_pair();
    let peer = std::thread::spawn(move || {
        let hello = receive(&peer);
        send(&peer, &hello);
        for seq in 2..=3 {
            let encoded = receive(&peer);
            let request = Frame::decode(encoded.as_slice()).unwrap();
            let Some(Body::Command(command)) = request.body else {
                panic!()
            };
            let mut response = Frame {
                major: 1,
                minor: 0,
                seq,
                worker_instance: vec![1; 16],
                device_epoch: 4,
                body: Some(Body::CommandResult(CommandResult {
                    request_id: command.request_id,
                    result: Some(command_result::Result::Sent(true)),
                })),
            }
            .encode_to_vec();
            if seq == 3 {
                // Duplicate singular field is forbidden, even if the prost would accept it.
                response.extend([0x18, 0x03]);
            }
            send(&peer, &response);
        }
    });
    let mut client =
        TunnelClient::connect(fd, [1; 16], 4, [2; 16], [3; 16], Duration::from_secs(2)).unwrap();
    let command = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        payload: Some(Payload::Describe(Describe::default())),
        ..Default::default()
    };
    assert!(
        client
            .command(command.clone(), Duration::from_secs(2))
            .is_ok()
    );
    assert!(matches!(
        client.command(command.clone(), Duration::from_secs(2)),
        Err(TunnelError::Invalid)
    ));
    assert!(matches!(
        client.command(command, Duration::from_secs(2)),
        Err(TunnelError::Closed)
    ));
    peer.join().unwrap();
}

#[test]
fn command_uses_relay_sequence_and_rejects_direct_bind() {
    use carosign_protocol::carosign::provider::v1::{BindAuthenticator, Describe};
    let mut sender = TunnelCommandEncoder::after_hello([1; 16], 4, [2; 16]).unwrap();
    let frame = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        payload: Some(Payload::Describe(Describe::default())),
        ..Default::default()
    };
    let (bytes, id) = sender.command(frame).unwrap();
    let decoded = Frame::decode(&*bytes).unwrap();
    assert_eq!(decoded.seq, 2);
    assert_eq!(id[15], 0);
    let Some(Body::Command(command)) = decoded.body else {
        panic!()
    };
    assert_eq!(command.request_id, id);
    assert_eq!(command.frame.unwrap().frame_seq, 0);
    let bind = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 4,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator::default())),
        ..Default::default()
    };
    assert_eq!(sender.command(bind), Err(HelloError::Invalid));
    assert!(sender.command(ProviderFrame::default()).is_err());
}

#[test]
fn provider_channel_sends_through_tunnel() {
    use carosign_adapters::provider::ProviderChannel;
    use carosign_protocol::carosign::broker_relay::v1::CommandResult;
    use carosign_protocol::carosign::provider::v1::Describe;
    let (fd, peer) = socket_pair();
    let server = std::thread::spawn(move || {
        send(&peer, &receive(&peer));
        let request = Frame::decode(receive(&peer).as_slice()).unwrap();
        let Some(Body::Command(command)) = request.body else {
            panic!()
        };
        let response = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 4,
            body: Some(Body::CommandResult(CommandResult {
                request_id: command.request_id,
                result: Some(command_result::Result::Sent(true)),
            })),
        };
        send(&peer, &response.encode_to_vec());
    });
    let client =
        TunnelClient::connect(fd, [1; 16], 4, [2; 16], [3; 16], Duration::from_secs(2)).unwrap();
    let channel = TunnelProviderChannel::new(client, Duration::from_secs(2));
    channel
        .send(ProviderFrame {
            worker_instance_id: vec![1; 16],
            device_epoch: 4,
            payload: Some(Payload::Describe(Describe::default())),
            ..Default::default()
        })
        .unwrap();
    server.join().unwrap();
}

#[test]
fn control_is_scoped_and_rejects_reused_cancel_id() {
    use carosign_protocol::carosign::broker_relay::v1::{Claim, Owner};
    let worker = [1; 16];
    let scope = Scope {
        lease_id: vec![3; 16],
        owner: Some(Owner {
            operation_id: vec![4; 16],
            worker_instance: worker.to_vec(),
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: vec![2; 16],
            mode: 1,
        }],
    };
    let mut encoder = TunnelCommandEncoder::after_hello(worker, 7, [8; 16]).unwrap();
    let (bytes, id) = encoder.control(scope.clone(), Some([9; 16])).unwrap();
    let frame = Frame::decode(bytes.as_slice()).unwrap();
    assert_eq!(frame.seq, 2);
    let Some(Body::Control(message)) = frame.body else {
        panic!("ожидался Control")
    };
    assert_eq!(message.request_id, id);
    let Some(control::Action::Cancel(cancel)) = message.action else {
        panic!("ожидался Cancel")
    };
    assert_eq!(cancel.scope, Some(scope.clone()));
    assert_eq!(cancel.cancel_command_id, vec![9; 16]);
    let (bytes, _) = encoder.control(scope.clone(), None).unwrap();
    let frame = Frame::decode(bytes.as_slice()).unwrap();
    assert!(matches!(
        frame.body,
        Some(Body::Control(Control {
            action: Some(control::Action::Revoke(_)),
            ..
        }))
    ));
    assert!(encoder.control(scope, Some([9; 16])).is_err());
    assert!(matches!(
        encoder.control(Scope::default(), None),
        Err(HelloError::Closed)
    ));
}

#[test]
fn bind_request_is_scoped_and_rejects_mismatched_slots() {
    use carosign_protocol::carosign::broker_relay::v1::{Claim, Owner};
    use carosign_protocol::carosign::common::v1::AuthenticatorRef;
    use carosign_protocol::carosign::provider::v1::{BindAuthenticator, FdSlot};
    let worker = [1; 16];
    let resource = vec![2; 16];
    let scope = Scope {
        lease_id: vec![3; 16],
        owner: Some(Owner {
            operation_id: vec![4; 16],
            worker_instance: worker.to_vec(),
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: resource.clone(),
            mode: 1,
        }],
    };
    let slots = vec![Slot {
        resource_id: resource.clone(),
        role: 1,
    }];
    let frame = ProviderFrame {
        worker_instance_id: worker.to_vec(),
        device_epoch: 7,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            authenticator: Some(AuthenticatorRef {
                authenticator_id: vec![5; 16],
                device_epoch: 7,
            }),
            profile_id: vec![6; 16],
            lease_id: vec![3; 16],
            fd_slots: vec![FdSlot {
                index: 0,
                role: 1,
                resource_id: resource,
            }],
        })),
        ..Default::default()
    };
    let mut encoder = TunnelCommandEncoder::after_hello(worker, 7, [8; 16]).unwrap();
    let (bytes, id) = encoder
        .bind(scope.clone(), slots.clone(), frame.clone())
        .unwrap();
    let relay = Frame::decode(bytes.as_slice()).unwrap();
    assert_eq!(relay.seq, 2);
    let Some(Body::Bind(bind)) = relay.body else {
        panic!("ожидался Bind")
    };
    assert_eq!(bind.request_id, id);
    assert_eq!(bind.scope, Some(scope.clone()));
    let mut mismatched = slots;
    mismatched[0].role = 2;
    assert!(encoder.bind(scope, mismatched, frame).is_err());
    assert!(matches!(
        encoder.command(ProviderFrame::default()),
        Err(HelloError::Closed)
    ));
}

#[test]
fn bind_times_out_when_broker_does_not_reply() {
    use carosign_protocol::carosign::broker_relay::v1::{Claim, Owner};
    use carosign_protocol::carosign::common::v1::AuthenticatorRef;
    use carosign_protocol::carosign::provider::v1::{BindAuthenticator, FdSlot};
    let (fd, peer) = socket_pair();
    let (release, wait) = std::sync::mpsc::channel();
    let broker = std::thread::spawn(move || {
        send(&peer, &receive(&peer));
        let request = Frame::decode(receive(&peer).as_slice()).unwrap();
        assert!(matches!(request.body, Some(Body::Bind(_))));
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    let mut client =
        TunnelClient::connect(fd, [1; 16], 7, [8; 16], [9; 16], Duration::from_secs(2)).unwrap();
    let scope = Scope {
        lease_id: vec![3; 16],
        owner: Some(Owner {
            operation_id: vec![4; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: vec![2; 16],
            mode: 1,
        }],
    };
    let slots = vec![Slot {
        resource_id: vec![2; 16],
        role: 1,
    }];
    let frame = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 7,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            authenticator: Some(AuthenticatorRef {
                authenticator_id: vec![5; 16],
                device_epoch: 7,
            }),
            profile_id: vec![6; 16],
            lease_id: vec![3; 16],
            fd_slots: vec![FdSlot {
                index: 0,
                role: 1,
                resource_id: vec![2; 16],
            }],
        })),
        ..Default::default()
    };
    let started = Instant::now();
    assert!(matches!(
        client.bind(scope, slots, frame, Duration::from_millis(80)),
        Err(TunnelError::Timeout)
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
    release.send(()).unwrap();
    broker.join().unwrap();
}

#[test]
fn scoped_revoke_requires_matching_positive_receipt() {
    use carosign_protocol::carosign::broker_relay::v1::{Claim, ControlResult, Owner};
    for positive in [true, false] {
        let (fd, peer) = socket_pair();
        let child = std::thread::spawn(move || {
            send(&peer, &receive(&peer));
            let request = Frame::decode(receive(&peer).as_slice()).unwrap();
            let Some(Body::Control(control)) = request.body else {
                panic!("ожидался Control")
            };
            assert!(matches!(control.action, Some(control::Action::Revoke(_))));
            let answer = Frame {
                major: 1,
                minor: 0,
                seq: 2,
                worker_instance: vec![1; 16],
                device_epoch: 7,
                body: Some(Body::ControlResult(ControlResult {
                    request_id: control.request_id,
                    result: Some(control_result::Result::Revoked(positive)),
                })),
            };
            send(&peer, &answer.encode_to_vec());
        });
        let mut client =
            TunnelClient::connect(fd, [1; 16], 7, [8; 16], [9; 16], Duration::from_secs(2))
                .unwrap();
        let scope = Scope {
            lease_id: vec![3; 16],
            owner: Some(Owner {
                operation_id: vec![4; 16],
                worker_instance: vec![1; 16],
                authenticator_id: vec![5; 16],
                device_epoch: 7,
            }),
            claims: vec![Claim {
                resource_id: vec![2; 16],
                mode: 1,
            }],
        };
        assert_eq!(
            client.revoke_scoped(scope, Duration::from_secs(2)).is_ok(),
            positive
        );
        child.join().unwrap();
    }
}

#[test]
fn scoped_cancel_waits_for_matching_receipt_and_rejects_reuse() {
    use carosign_protocol::carosign::broker_relay::v1::{Claim, ControlResult, Owner};
    let (fd, peer) = socket_pair();
    let child = std::thread::spawn(move || {
        send(&peer, &receive(&peer));
        let request = Frame::decode(receive(&peer).as_slice()).unwrap();
        let Some(Body::Control(control)) = request.body else {
            panic!("ожидался Control")
        };
        let Some(control::Action::Cancel(cancel)) = control.action else {
            panic!("ожидалась отмена")
        };
        assert_eq!(cancel.cancel_command_id, vec![9; 16]);
        let answer = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 7,
            body: Some(Body::ControlResult(ControlResult {
                request_id: control.request_id,
                result: Some(control_result::Result::CancelSent(true)),
            })),
        };
        send(&peer, &answer.encode_to_vec());
    });
    let client =
        TunnelClient::connect(fd, [1; 16], 7, [8; 16], [9; 16], Duration::from_secs(2)).unwrap();
    let client = TunnelProviderChannel::new(client, Duration::from_secs(2));
    let scope = Scope {
        lease_id: vec![3; 16],
        owner: Some(Owner {
            operation_id: vec![4; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: vec![2; 16],
            mode: 1,
        }],
    };
    client.cancel_scoped(scope.clone(), [9; 16]).unwrap();
    assert!(matches!(
        client.cancel_scoped(scope, [9; 16]),
        Err(TunnelError::Invalid)
    ));
    child.join().unwrap();
}

#[test]
fn bind_over_socket_accepts_only_matching_sent_receipt() {
    use carosign_protocol::carosign::broker_relay::v1::{BindResult, Claim, Event, Owner};
    use carosign_protocol::carosign::common::v1::{AuthenticatorRef, CapabilityDescriptor};
    use carosign_protocol::carosign::provider::v1::{
        AuthenticatorReady, BindAuthenticator, FdSlot,
    };
    let (fd, peer) = socket_pair();
    let child = std::thread::spawn(move || {
        send(&peer, &receive(&peer));
        let request = Frame::decode(receive(&peer).as_slice()).unwrap();
        let Some(Body::Bind(bind)) = request.body else {
            panic!("ожидался Bind")
        };
        let answer = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 7,
            body: Some(Body::BindResult(BindResult {
                request_id: bind.request_id,
                result: Some(bind_result::Result::Sent(true)),
            })),
        };
        send(&peer, &answer.encode_to_vec());
        let ready = Frame {
            major: 1,
            minor: 0,
            seq: 3,
            worker_instance: vec![1; 16],
            device_epoch: 7,
            body: Some(Body::Event(Event {
                frame: Some(ProviderFrame {
                    major: 1,
                    minor: 3,
                    worker_instance_id: vec![1; 16],
                    frame_seq: 1,
                    device_epoch: 7,
                    payload: Some(Payload::AuthenticatorReady(AuthenticatorReady {
                        authenticator: Some(AuthenticatorRef {
                            authenticator_id: vec![5; 16],
                            device_epoch: 7,
                        }),
                        capabilities: Some(CapabilityDescriptor::default()),
                    })),
                    ..Default::default()
                }),
            })),
        };
        send(&peer, &ready.encode_to_vec());
    });
    let mut client =
        TunnelClient::connect(fd, [1; 16], 7, [8; 16], [9; 16], Duration::from_secs(2)).unwrap();
    let scope = Scope {
        lease_id: vec![3; 16],
        owner: Some(Owner {
            operation_id: vec![4; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![Claim {
            resource_id: vec![2; 16],
            mode: 1,
        }],
    };
    let slots = vec![Slot {
        resource_id: vec![2; 16],
        role: 1,
    }];
    let frame = ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 7,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            authenticator: Some(AuthenticatorRef {
                authenticator_id: vec![5; 16],
                device_epoch: 7,
            }),
            profile_id: vec![6; 16],
            lease_id: vec![3; 16],
            fd_slots: vec![FdSlot {
                index: 0,
                role: 1,
                resource_id: vec![2; 16],
            }],
        })),
        ..Default::default()
    };
    client
        .bind(scope, slots, frame, Duration::from_secs(2))
        .unwrap();
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(2))
            .unwrap()
            .payload,
        Some(Payload::AuthenticatorReady(_))
    ));
    assert!(client.pending_authenticator.is_none());
    child.join().unwrap();
}

#[test]
fn unsolicited_authenticator_ready_closes_tunnel() {
    use carosign_protocol::carosign::broker_relay::v1::Event;
    use carosign_protocol::carosign::common::v1::{AuthenticatorRef, CapabilityDescriptor};
    use carosign_protocol::carosign::provider::v1::AuthenticatorReady;
    let (fd, peer) = socket_pair();
    let child = std::thread::spawn(move || {
        send(&peer, &receive(&peer));
        let ready = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 7,
            body: Some(Body::Event(Event {
                frame: Some(ProviderFrame {
                    major: 1,
                    minor: 3,
                    worker_instance_id: vec![1; 16],
                    frame_seq: 1,
                    device_epoch: 7,
                    payload: Some(Payload::AuthenticatorReady(AuthenticatorReady {
                        authenticator: Some(AuthenticatorRef {
                            authenticator_id: vec![5; 16],
                            device_epoch: 7,
                        }),
                        capabilities: Some(CapabilityDescriptor::default()),
                    })),
                    ..Default::default()
                }),
            })),
        };
        send(&peer, &ready.encode_to_vec());
    });
    let mut client =
        TunnelClient::connect(fd, [1; 16], 7, [8; 16], [9; 16], Duration::from_secs(2)).unwrap();
    assert!(matches!(
        client.receive_event(Duration::from_secs(2)),
        Err(TunnelError::Invalid)
    ));
    assert!(matches!(
        client.receive_event(Duration::from_secs(2)),
        Err(TunnelError::Closed)
    ));
    child.join().unwrap();
}

#[test]
fn hello_rejects_replay_unknown_and_wrong_context() {
    let mut expected = TunnelHello::new([1; 16], 4, [2; 16], [3; 16]).unwrap();
    let encoded = expected.outgoing().unwrap();
    assert_eq!(expected.accept(&encoded, 0), Ok(()));
    assert_eq!(expected.accept(&encoded, 0), Err(HelloError::Closed));
    let mut unknown = encoded.clone();
    unknown.extend([0x98, 0x06, 0x01]);
    let mut bad = TunnelHello::new([1; 16], 4, [2; 16], [3; 16]).unwrap();
    bad.outgoing().unwrap();
    assert_eq!(bad.accept(&unknown, 0), Err(HelloError::Invalid));
    let mut wrong = TunnelHello::new([1; 16], 5, [2; 16], [3; 16]).unwrap();
    wrong.outgoing().unwrap();
    assert_eq!(wrong.accept(&encoded, 0), Err(HelloError::Invalid));
    let mut fds = TunnelHello::new([1; 16], 4, [2; 16], [3; 16]).unwrap();
    fds.outgoing().unwrap();
    assert_eq!(fds.accept(&encoded, 1), Err(HelloError::Invalid));
}

#[test]
fn typed_progress_requires_negotiation_and_bound_capability() {
    use carosign_protocol::carosign::{
        broker_relay::v1::Event,
        common::v1::{CaptureEventKind, CaptureFeedback, Modality, capture_feedback::Kind},
        provider::v1::ProviderProgress,
    };
    for (negotiated, declared, valid) in [
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let (fd, _peer) = socket_pair();
        let mut client = TunnelClient {
            fd,
            encoder: TunnelCommandEncoder::after_hello([1; 16], 4, [3; 16]).unwrap(),
            worker: [1; 16],
            epoch: 4,
            next_in: 2,
            provider_in: carosign_provider_sdk::wire::Channel::new(),
            pending_events: VecDeque::new(),
            pending_authenticator: None,
            pending_prebind: None,
            pending_challenge: None,
            capture_feedback_enabled: negotiated,
            capture_device: Some(CapabilityDescriptor {
                modality: Modality::Fingerprint as i32,
                supported_events: if declared {
                    vec![CaptureEventKind::CaptureEventProgress as i32]
                } else {
                    Vec::new()
                },
                ..Default::default()
            }),
            closed: false,
        };
        let frame = ProviderFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            worker_instance_id: vec![1; 16],
            frame_seq: 1,
            operation_id: vec![4; 16],
            command_id: vec![5; 16],
            device_epoch: 4,
            payload: Some(Payload::ProviderProgress(ProviderProgress {
                operation_sequence: 1,
                feedback: Some(CaptureFeedback {
                    kind: Some(Kind::Percent(30)),
                }),
                ..Default::default()
            })),
            ..Default::default()
        };
        let bytes = Frame {
            major: 1,
            minor: 0,
            seq: 2,
            worker_instance: vec![1; 16],
            device_epoch: 4,
            body: Some(Body::Event(Event {
                frame: Some(frame.clone()),
            })),
        }
        .encode_to_vec();

        if valid {
            assert_eq!(client.parse_event(&bytes).unwrap(), frame);
        } else {
            assert!(matches!(
                client.parse_event(&bytes),
                Err(TunnelError::Invalid)
            ));
        }
    }
}
