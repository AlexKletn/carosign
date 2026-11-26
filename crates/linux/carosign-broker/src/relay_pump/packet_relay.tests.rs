use super::*;
use crate::relay_queue::QueueError;
use crate::transport::{PacketSocket, TransportError};
use carosign_linux::sys::socketpair_seqpacket;
use carosign_protocol::carosign::provider::v1::ProviderFrame;
use carosign_protocol::carosign::provider::v1::{Shutdown, provider_frame::Payload};
use prost::Message;

#[path = "fixtures/cancel_ids.rs"]
mod cancel_ids;
use cancel_ids::CancelIds;
#[path = "fixtures/unused_mutation.rs"]
mod unused_mutation;
use unused_mutation::UnusedMutation;

#[test]
fn oversized_bind_closes_session_instead_of_leaving_it_usable() {
    use carosign_protocol::carosign::provider::v1::BindAuthenticator;
    use std::os::fd::AsFd;

    let provider = socketpair_seqpacket().unwrap();
    let tunnel = socketpair_seqpacket().unwrap();
    let mut pump = RelayPump::new(
        PacketSocket::new(provider.platform).unwrap(),
        PacketSocket::new(tunnel.platform).unwrap(),
    );
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; carosign_protocol::limits::FRAME_MAX],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        uid: None,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator::default())),
    };

    assert_eq!(
        pump.sequenced_bind(frame, vec![]),
        Err(QueueError::InvalidFrame)
    );
    assert_eq!(pump.command(vec![1]), Err(QueueError::Closed));
    assert_peer_closed(provider.worker.as_fd());
    assert_peer_closed(tunnel.worker.as_fd());
}

fn assert_peer_closed(fd: std::os::fd::BorrowedFd<'_>) {
    use std::os::fd::AsRawFd;
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN | libc::POLLHUP,
        revents: 0,
    };
    // With a parallel fork, the test process can briefly hold FD to exec with CLOEXEC; expect
    // real closing with a limited timeframe.
    assert_eq!(unsafe { libc::poll(&mut poll, 1, 2000) }, 1);
    let mut byte = 0u8;
    assert_eq!(
        unsafe {
            libc::recv(
                fd.as_raw_fd(),
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_DONTWAIT,
            )
        },
        0
    );
}

#[test]
fn bind_fds_follow_control_priority_and_survive_eagain() {
    use carosign_protocol::carosign::provider::v1::{BindAuthenticator, FdRole, FdSlot, RequestCancel};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    let provider = socketpair_seqpacket().unwrap();
    let tunnel = socketpair_seqpacket().unwrap();
    let device = socketpair_seqpacket().unwrap();
    let mut pump = RelayPump::new(
        PacketSocket::new(provider.platform).unwrap(),
        PacketSocket::new(tunnel.platform).unwrap(),
    );
    let raw = provider.worker.as_raw_fd();
    let frame = |payload| ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![4; 16],
        device_epoch: 7,
        uid: matches!(
            &payload,
            Payload::StartVerification(_)
                | Payload::StartEnrollment(_)
                | Payload::ContinueEnrollment(_)
                | Payload::StartRemoval(_)
                | Payload::PrepareEnrollmentChallenge(_)
        )
        .then_some(1000),
        payload: Some(payload),
    };
    let bind = frame(Payload::BindAuthenticator(BindAuthenticator {
        fd_slots: vec![FdSlot {
            index: 0,
            role: FdRole::Device as i32,
            resource_id: vec![5; 16],
        }],
        ..Default::default()
    }));
    pump.sequenced_bind(bind, vec![device.platform]).unwrap();
    pump.sequenced_control(frame(Payload::RequestCancel(RequestCancel {
        target_operation_id: vec![4; 16],
    })))
    .unwrap();
    let mut seq = 1;
    while pump.provider_for_test().send(&[7; 1024]).is_ok() {}
    assert!(matches!(
        pump.send_sequenced_command(true, &mut seq),
        Err(TransportError::WouldBlock)
    ));
    assert_eq!(seq, 2);
    let mut buf = [0u8; 4096];
    while unsafe { libc::recv(raw, buf.as_mut_ptr().cast(), buf.len(), libc::MSG_DONTWAIT) } > 0 {}
    assert!(pump.send_sequenced_command(true, &mut seq).unwrap());
    let n = unsafe { libc::recv(raw, buf.as_mut_ptr().cast(), buf.len(), 0) };
    let control = ProviderFrame::decode(&buf[..n as usize]).unwrap();
    assert_eq!(control.frame_seq, 1);
    assert!(matches!(control.payload, Some(Payload::RequestCancel(_))));
    while pump.provider_for_test().send(&[7; 1024]).is_ok() {}
    assert!(matches!(
        pump.send_sequenced_command(true, &mut seq),
        Err(TransportError::WouldBlock)
    ));
    assert_eq!(seq, 3, "заблокированный Bind удерживает назначенный номер");
    while unsafe { libc::recv(raw, buf.as_mut_ptr().cast(), buf.len(), libc::MSG_DONTWAIT) } > 0 {}
    assert!(pump.send_sequenced_command(true, &mut seq).unwrap());
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    let space = unsafe { libc::CMSG_SPACE(std::mem::size_of::<libc::c_int>() as u32) as usize };
    let mut ancillary = vec![0usize; space.div_ceil(std::mem::size_of::<usize>())];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = ancillary.as_mut_ptr().cast();
    msg.msg_controllen = space;
    let n = unsafe { libc::recvmsg(raw, &mut msg, libc::MSG_CMSG_CLOEXEC) };
    assert!(n > 0);
    assert_eq!(msg.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC), 0);
    let received_frame = ProviderFrame::decode(&buf[..n as usize]).unwrap();
    assert_eq!(received_frame.frame_seq, 2);
    assert!(matches!(
        received_frame.payload,
        Some(Payload::BindAuthenticator(_))
    ));
    let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    assert!(!cmsg.is_null());
    assert_eq!(unsafe { (*cmsg).cmsg_type }, libc::SCM_RIGHTS);
    let fd = unsafe { OwnedFd::from_raw_fd(*libc::CMSG_DATA(cmsg).cast::<libc::c_int>()) };
    assert_eq!(
        unsafe { libc::send(fd.as_raw_fd(), b"x".as_ptr().cast(), 1, libc::MSG_NOSIGNAL) },
        1
    );
    assert_eq!(
        unsafe { libc::recv(device.worker.as_raw_fd(), buf.as_mut_ptr().cast(), 1, 0) },
        1
    );
    assert_eq!(buf[0], b'x');
    assert_eq!(seq, 3);
}

#[test]
fn bind_rejects_duplicate_resource_and_closes_all_descriptors() {
    use carosign_protocol::carosign::provider::v1::{BindAuthenticator, FdRole, FdSlot};

    let (mut pump, _, _) = sockets();
    let first = socketpair_seqpacket().unwrap();
    let second = socketpair_seqpacket().unwrap();
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![],
        device_epoch: 7,
        uid: None,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            fd_slots: (0..2)
                .map(|index| FdSlot {
                    index,
                    role: FdRole::Device as i32,
                    resource_id: vec![5; 16],
                })
                .collect(),
            ..Default::default()
        })),
    };
    assert!(matches!(
        pump.sequenced_bind(frame, vec![first.platform, second.platform]),
        Err(QueueError::InvalidFrame)
    ));
    use std::os::fd::AsFd;
    for peer in [&first.worker, &second.worker] {
        assert_peer_closed(peer.as_fd());
    }
}

#[test]
fn revoke_closes_queued_bind_descriptors() {
    use carosign_protocol::carosign::provider::v1::{BindAuthenticator, FdRole, FdSlot};
    use std::os::fd::AsFd;
    let (mut pump, _worker, _daemon) = sockets();
    let device = socketpair_seqpacket().unwrap();
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![],
        device_epoch: 7,
        uid: None,
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            fd_slots: vec![FdSlot {
                index: 0,
                role: FdRole::Device as i32,
                resource_id: vec![5; 16],
            }],
            ..Default::default()
        })),
    };
    pump.sequenced_bind(frame.clone(), vec![device.platform])
        .unwrap();
    pump.revoke();
    assert_peer_closed(device.worker.as_fd());

    let (mut pump, _worker, _daemon) = sockets();
    let device = socketpair_seqpacket().unwrap();
    pump.sequenced_bind(frame, vec![device.platform]).unwrap();
    while pump.provider_for_test().send(&[7; 1024]).is_ok() {}
    let mut seq = 1;
    assert!(matches!(
        pump.send_sequenced_command(true, &mut seq),
        Err(TransportError::WouldBlock)
    ));
    pump.revoke();
    assert_peer_closed(device.worker.as_fd());
}

#[test]
fn controls_are_prioritized_and_accounted_separately() {
    let (mut pump, provider, _) = sockets();
    pump.command(vec![1]).unwrap();
    pump.control(vec![2]).unwrap();
    assert!(matches!(pump.send_command(true), Ok(true)));
    assert_eq!(provider.receive().unwrap(), vec![2]);
    assert!(matches!(pump.send_command(true), Ok(true)));
    assert_eq!(provider.receive().unwrap(), vec![1]);
    assert!(matches!(pump.send_command(true), Ok(false)));
    for _ in 0..8 {
        pump.control(vec![3]).unwrap();
    }
    assert_eq!(pump.control(vec![4]), Err(QueueError::Closed));
    assert!(matches!(
        pump.send_command(true),
        Err(TransportError::Closed)
    ));
}

#[test]
fn received_control_is_prioritized_and_invalid_control_closes_session() {
    let (mut pump, provider, tunnel) = sockets();
    pump.command(vec![1]).unwrap();
    tunnel.send(&[2]).unwrap();
    assert!(matches!(
        pump.receive_request(|bytes| Ok(CheckedRequest::Control { request_id: [2; 16], bytes: bytes.to_vec() })),
        Ok(ReceiveRequest::Queued { request_id }) if request_id == [2; 16]
    ));
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![2]);
    assert_eq!(pump.take_sent_request_id(), Some([2; 16]));
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![1]);

    tunnel.send(&[3]).unwrap();
    assert!(matches!(
        pump.receive_request(|_| Err(())),
        Err(TransportError::Invalid)
    ));
    assert!(matches!(
        pump.send_command(true),
        Err(TransportError::Closed)
    ));
}

#[test]
fn full_command_queue_reports_busy_but_still_accepts_control() {
    let (mut pump, provider, tunnel) = sockets();
    for _ in 0..64 {
        pump.command(vec![1]).unwrap();
    }
    tunnel.send(&[1]).unwrap();
    assert!(matches!(
        pump.receive_request(|bytes| Ok(CheckedRequest::Command { request_id: [1; 16], bytes: bytes.to_vec() })),
        Ok(ReceiveRequest::Busy { request_id }) if request_id == [1; 16]
    ));
    tunnel.send(&[2]).unwrap();
    assert!(matches!(
        pump.receive_request(|bytes| Ok(CheckedRequest::Control { request_id: [2; 16], bytes: bytes.to_vec() })),
        Ok(ReceiveRequest::Queued { request_id }) if request_id == [2; 16]
    ));
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![2]);
}

#[test]
fn send_receipt_preserves_request_identity_across_priority_and_close() {
    let (mut pump, provider, tunnel) = sockets();
    tunnel.send(&[1]).unwrap();
    assert_eq!(
        pump.receive_request(|bytes| Ok(CheckedRequest::Command {
            request_id: [1; 16],
            bytes: bytes.to_vec(),
        }))
        .unwrap(),
        ReceiveRequest::Queued {
            request_id: [1; 16]
        },
    );
    tunnel.send(&[2]).unwrap();
    assert_eq!(
        pump.receive_request(|bytes| Ok(CheckedRequest::Control {
            request_id: [2; 16],
            bytes: bytes.to_vec(),
        }))
        .unwrap(),
        ReceiveRequest::Queued {
            request_id: [2; 16]
        },
    );
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![2]);
    // The owner is obliged to collect the receipt before the next shipment.
    assert!(matches!(
        pump.send_command(true),
        Err(TransportError::WouldBlock)
    ));
    assert_eq!(pump.take_sent_request_id(), Some([2; 16]));
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![1]);
    // A review after a successful send should not destroy his receipt.
    pump.revoke();
    assert_eq!(pump.take_sent_request_id(), Some([1; 16]));
    assert_eq!(pump.take_sent_request_id(), None);
}

#[test]
fn command_result_is_queued_only_after_send_and_keeps_request_id() {
    use carosign_protocol::carosign::broker_relay::v1::{Frame, command_result, frame::Body};
    let (mut pump, provider, tunnel) = sockets();
    let mut next = 4;
    assert!(!pump.queue_sent_result([1; 16], 7, &mut next).unwrap());
    assert_eq!(next, 4);
    tunnel.send(&[9]).unwrap();
    pump.receive_request(|bytes| {
        Ok(CheckedRequest::Command {
            request_id: [9; 16],
            bytes: bytes.to_vec(),
        })
    })
    .unwrap();
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![9]);
    assert!(pump.queue_sent_result([1; 16], 7, &mut next).unwrap());
    assert_eq!(next, 5);
    assert!(!pump.queue_sent_result([1; 16], 7, &mut next).unwrap());
    assert!(pump.send_event().unwrap());
    let frame = Frame::decode(tunnel.receive().unwrap().as_slice()).unwrap();
    assert_eq!(frame.seq, 4);
    assert_eq!(frame.worker_instance, vec![1; 16]);
    assert_eq!(frame.device_epoch, 7);
    let Some(Body::CommandResult(result)) = frame.body else {
        panic!("ожидался ответ на команду");
    };
    assert_eq!(result.request_id, vec![9; 16]);
    assert_eq!(result.result, Some(command_result::Result::Sent(true)));
}

#[test]
fn control_receipt_is_not_misreported_as_command_or_cancel() {
    let (mut pump, provider, tunnel) = sockets();
    tunnel.send(&[2]).unwrap();
    pump.receive_request(|bytes| {
        Ok(CheckedRequest::Control {
            request_id: [2; 16],
            bytes: bytes.to_vec(),
        })
    })
    .unwrap();
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![2]);
    let mut next = 4;
    assert_eq!(
        pump.queue_sent_result([1; 16], 7, &mut next),
        Err(QueueError::InvalidFrame)
    );
    assert_eq!(next, 4);
    assert_eq!(
        pump.take_sent_receipt(),
        Some(SentReceipt {
            request_id: [2; 16],
            is_control: true,
        })
    );
}

#[test]
fn blocked_send_does_not_issue_receipt_until_provider_send_succeeds() {
    let (mut pump, provider, tunnel) = sockets();
    let filler = vec![7; 4096];
    loop {
        match pump.provider_for_test().send(&filler) {
            Ok(()) => {}
            Err(TransportError::WouldBlock) => break,
            Err(error) => panic!("ошибка заполнения сокета: {error:?}"),
        }
    }
    tunnel.send(&[9]).unwrap();
    assert_eq!(
        pump.receive_request(|bytes| Ok(CheckedRequest::Command {
            request_id: [9; 16],
            bytes: bytes.to_vec(),
        }))
        .unwrap(),
        ReceiveRequest::Queued {
            request_id: [9; 16]
        },
    );
    assert!(matches!(
        pump.send_command(true),
        Err(TransportError::WouldBlock)
    ));
    assert_eq!(pump.take_sent_request_id(), None);
    while provider.receive().is_ok() {}
    assert!(pump.send_command(true).unwrap());
    assert_eq!(provider.receive().unwrap(), vec![9]);
    assert_eq!(pump.take_sent_request_id(), Some([9; 16]));
}

fn sockets() -> (RelayPump, PacketSocket, PacketSocket) {
    let provider = socketpair_seqpacket().unwrap();
    let tunnel = socketpair_seqpacket().unwrap();
    (
        RelayPump::new(
            PacketSocket::new(provider.platform).unwrap(),
            PacketSocket::new(tunnel.platform).unwrap(),
        ),
        PacketSocket::new(provider.worker).unwrap(),
        PacketSocket::new(tunnel.worker).unwrap(),
    )
}

/// Verifies the actual individual processes of the broker and mock provider.
/// The test examiner only admits Hello and Initialize; this is not a substitute for
/// production-testing wire and authority.
#[test]
#[ignore = "нужен путь к собранному mock в CAROSIGN_MOCK_PROVIDER_TEST_BINARY"]
fn separate_broker_initializes_real_mock() {
    use carosign_protocol::carosign::broker_relay::v1::{
        BindResult, CommandResult, ControlResult, Event, Frame, Hello, bind_result, command_result,
        control, control_result, frame::Body,
    };
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    const TUNNEL: &str = "CAROSIGN_INIT_RELAY_TUNNEL";
    const PROVIDER: &str = "CAROSIGN_INIT_RELAY_PROVIDER";
    const CLIENT: &str = "CAROSIGN_INIT_RELAY_CLIENT";
    if let Ok(fd) = std::env::var(CLIENT) {
        // SAFETY: The test parent only transmits the tunnel descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(fd.parse().unwrap()) };
        run_handshake_client(fd);
        return;
    }
    if let (Ok(tunnel), Ok(provider)) = (std::env::var(TUNNEL), std::env::var(PROVIDER)) {
        // SAFETY: The descriptors are transmitted only by the test parent.
        let tunnel =
            PacketSocket::new(unsafe { OwnedFd::from_raw_fd(tunnel.parse().unwrap()) }).unwrap();
        let provider =
            PacketSocket::new(unsafe { OwnedFd::from_raw_fd(provider.parse().unwrap()) }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let received = loop {
            match tunnel.receive() {
                Ok(packet) => break packet,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                other => panic!("получение Hello: {other:?}"),
            }
        };
        assert!(matches!(
            Frame::decode(received.as_slice()).unwrap().body,
            Some(Body::Hello(_))
        ));
        tunnel
            .send(
                &Frame {
                    major: 1,
                    minor: 0,
                    seq: 1,
                    worker_instance: vec![1; 16],
                    device_epoch: 7,
                    body: Some(Body::Hello(Hello {
                        broker_instance: vec![3; 16],
                        boot_id: vec![4; 16],
                    })),
                }
                .encode_to_vec(),
            )
            .unwrap();
        let mut pump = RelayPump::new(provider, tunnel);
        let emit = |pump: &mut RelayPump, relay_seq, expected: fn(&Payload) -> bool| {
            loop {
                match pump.receive_event(|raw| {
                    let value = ProviderFrame::decode(raw).map_err(|_| ())?;
                    if !expected(value.payload.as_ref().ok_or(())?) {
                        return Err(());
                    }
                    Ok(Frame {
                        major: 1,
                        minor: 0,
                        seq: relay_seq,
                        worker_instance: vec![1; 16],
                        device_epoch: 7,
                        body: Some(Body::Event(Event { frame: Some(value) })),
                    }
                    .encode_to_vec())
                }) {
                    Ok(true) => break,
                    Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                        std::thread::yield_now()
                    }
                    other => panic!("событие mock: {other:?}"),
                }
            }
            assert!(pump.send_event().unwrap());
        };
        emit(&mut pump, 2, |p| matches!(p, Payload::WorkerHello(_)));
        let mut request = None;
        let mut provider_seq = 1;
        loop {
            match pump.receive_sequenced_command([1; 16], 7, &mut provider_seq, |raw| {
                let frame = Frame::decode(raw).map_err(|_| ())?;
                if frame.seq != 2 || frame.worker_instance != vec![1; 16] || frame.device_epoch != 7
                {
                    return Err(());
                }
                let Some(Body::Command(command)) = frame.body else {
                    return Err(());
                };
                let provider = command.frame.ok_or(())?;
                if !matches!(provider.payload, Some(Payload::Initialize(_))) {
                    return Err(());
                }
                request = Some(command.request_id);
                Ok(provider)
            }) {
                Ok(true) => break,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                other => panic!("команда Initialize: {other:?}"),
            }
        }
        assert!(
            pump.send_sequenced_command(true, &mut provider_seq)
                .unwrap()
        );
        pump.event(
            Frame {
                major: 1,
                minor: 0,
                seq: 3,
                worker_instance: vec![1; 16],
                device_epoch: 7,
                body: Some(Body::CommandResult(CommandResult {
                    request_id: request.unwrap(),
                    result: Some(command_result::Result::Sent(true)),
                })),
            }
            .encode_to_vec(),
        )
        .unwrap();
        assert!(pump.send_event().unwrap());
        emit(&mut pump, 4, |p| matches!(p, Payload::Initialized(_)));
        // Only the test owner allows an empty set of mock slots.
        // Production verification of issuance and opening of resources is absent.
        let packet = loop {
            match pump.tunnel_for_test().receive() {
                Ok(packet) => break packet,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                other => panic!("получение Bind: {other:?}"),
            }
        };
        let frame = Frame::decode(packet.as_slice()).unwrap();
        assert_eq!(frame.seq, 3);
        assert_eq!(frame.worker_instance, vec![1; 16]);
        assert_eq!(frame.device_epoch, 7);
        let Some(Body::Bind(bind)) = frame.body else {
            panic!("ожидался Bind");
        };
        assert!(bind.slots.is_empty());
        let scope = bind.scope.unwrap();
        assert_eq!(scope.lease_id, vec![6; 16]);
        let cancel_test = std::env::var_os("CAROSIGN_RELAY_CANCEL_TEST").is_some();
        assert_eq!(scope.owner.unwrap().operation_id, vec![12; 16]);
        let provider_frame = bind.frame.unwrap();
        let Some(Payload::BindAuthenticator(ref request)) = provider_frame.payload else {
            panic!("ожидался BindAuthenticator");
        };
        assert!(request.fd_slots.is_empty());
        assert_eq!(request.profile_id, vec![3; 16]);
        assert_eq!(request.lease_id, vec![6; 16]);
        assert_eq!(provider_frame.frame_seq, 0);
        pump.sequenced_bind(provider_frame, Vec::new()).unwrap();
        assert!(
            pump.send_sequenced_command(true, &mut provider_seq)
                .unwrap()
        );
        pump.event(
            Frame {
                major: 1,
                minor: 0,
                seq: 5,
                worker_instance: vec![1; 16],
                device_epoch: 7,
                body: Some(Body::BindResult(BindResult {
                    request_id: bind.request_id,
                    result: Some(bind_result::Result::Sent(true)),
                })),
            }
            .encode_to_vec(),
        )
        .unwrap();
        assert!(pump.send_event().unwrap());
        emit(&mut pump, 6, |p| {
            matches!(p, Payload::AuthenticatorReady(_))
        });
        let mut request = None;
        loop {
            match pump.receive_sequenced_command([1; 16], 7, &mut provider_seq, |raw| {
                let frame = Frame::decode(raw).map_err(|_| ())?;
                if frame.seq != 4 || frame.worker_instance != vec![1; 16] || frame.device_epoch != 7
                {
                    return Err(());
                }
                let Some(Body::Command(command)) = frame.body else {
                    return Err(());
                };
                let provider = command.frame.ok_or(())?;
                if !matches!(provider.payload, Some(Payload::StartVerification(_))) {
                    return Err(());
                }
                request = Some(command.request_id);
                Ok(provider)
            }) {
                Ok(true) => break,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                other => panic!("команда проверки: {other:?}"),
            }
        }
        assert!(
            pump.send_sequenced_command(true, &mut provider_seq)
                .unwrap()
        );
        pump.event(
            Frame {
                major: 1,
                minor: 0,
                seq: 7,
                worker_instance: vec![1; 16],
                device_epoch: 7,
                body: Some(Body::CommandResult(CommandResult {
                    request_id: request.unwrap(),
                    result: Some(command_result::Result::Sent(true)),
                })),
            }
            .encode_to_vec(),
        )
        .unwrap();
        assert!(pump.send_event().unwrap());
        emit(&mut pump, 8, |p| matches!(p, Payload::CommandAck(_)));
        if cancel_test {
            let packet = loop {
                match pump.tunnel_for_test().receive() {
                    Ok(packet) => break packet,
                    Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                        std::thread::yield_now()
                    }
                    other => panic!("получение Control: {other:?}"),
                }
            };
            let frame = Frame::decode(packet.as_slice()).unwrap();
            assert_eq!(frame.seq, 5);
            assert_eq!(frame.worker_instance, vec![1; 16]);
            assert_eq!(frame.device_epoch, 7);
            let Some(Body::Control(request)) = frame.body else {
                panic!("ожидался Control")
            };
            let Some(control::Action::Cancel(cancel)) = request.action else {
                panic!("ожидалась отмена")
            };
            let scope = cancel.scope.unwrap();
            assert_eq!(scope.lease_id, vec![6; 16]);
            let owner = scope.owner.unwrap();
            assert_eq!(owner.operation_id, vec![12; 16]);
            assert_eq!(owner.worker_instance, vec![1; 16]);
            assert_eq!(owner.authenticator_id, vec![5; 16]);
            assert_eq!(owner.device_epoch, 7);
            assert_eq!(scope.claims.len(), 1);
            assert_eq!(scope.claims[0].resource_id, vec![9; 16]);
            assert_eq!(scope.claims[0].mode, 1);
            assert_eq!(cancel.cancel_command_id, vec![22; 16]);
            let provider = ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                worker_instance_id: vec![1; 16],
                frame_seq: 0,
                command_id: cancel.cancel_command_id,
                operation_id: owner.operation_id.clone(),
                device_epoch: 7,
                uid: None,
                payload: Some(Payload::RequestCancel(
                    carosign_protocol::carosign::provider::v1::RequestCancel {
                        target_operation_id: owner.operation_id,
                    },
                )),
            };
            // The test owner has already checked the area and the disposable identifier,
            // transmitting the cancellation through the same priority queue that holds the
            // packet at reverse pressure.
            pump.sequenced_control(provider).unwrap();
            loop {
                match pump.send_sequenced_command(true, &mut provider_seq) {
                    Ok(true) => break,
                    Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                        std::thread::yield_now();
                    }
                    other => panic!("отправка Control: {other:?}"),
                }
            }
            pump.event(
                Frame {
                    major: 1,
                    minor: 0,
                    seq: 9,
                    worker_instance: vec![1; 16],
                    device_epoch: 7,
                    body: Some(Body::ControlResult(ControlResult {
                        request_id: request.request_id,
                        result: Some(control_result::Result::CancelSent(true)),
                    })),
                }
                .encode_to_vec(),
            )
            .unwrap();
            assert!(pump.send_event().unwrap());
            emit(&mut pump, 10, |p| matches!(p, Payload::CancelReceipt(_)));
            emit(&mut pump, 11, |p| {
                matches!(p, Payload::VerificationOutcome(_))
            });
            return;
        }
        emit(&mut pump, 9, |p| {
            matches!(p, Payload::VerificationOutcome(_))
        });
        return;
    }

    let mock = std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY").expect("путь к mock");
    let tunnel = socketpair_seqpacket().unwrap();
    let provider = socketpair_seqpacket().unwrap();
    for fd in [tunnel.worker.as_raw_fd(), provider.platform.as_raw_fd()] {
        // SAFETY: test-based descriptors.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    let mut broker_command = if let Some(runner) = std::env::var_os("CAROSIGN_RELAY_TEST_RUNNER") {
        let mut command = std::process::Command::new(runner);
        command.arg(tunnel.worker.as_raw_fd().to_string());
        command.arg(provider.platform.as_raw_fd().to_string());
        command
    } else {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "relay_pump::tests::separate_broker_initializes_real_mock",
            "--ignored",
            "--nocapture",
        ]);
        command.env(TUNNEL, tunnel.worker.as_raw_fd().to_string());
        command.env(PROVIDER, provider.platform.as_raw_fd().to_string());
        command
    };
    let mut broker = broker_command.spawn().unwrap();
    drop(tunnel.worker);
    drop(provider.platform);
    let worker_fd = provider.worker.as_raw_fd();
    let mut worker = std::process::Command::new(mock);
    let scenario = if std::env::var_os("CAROSIGN_RELAY_CANCEL_TEST").is_some() {
        "seeded-pending-cancel"
    } else if std::env::var_os("CAROSIGN_RELAY_MATCHED_TEST").is_some() {
        "seeded-matched-token"
    } else {
        "seeded-no-sample"
    };
    worker.args(["198", scenario]);
    // SAFETY: After fork, only dup2 is called.
    unsafe {
        worker.pre_exec(move || {
            if libc::dup2(worker_fd, 198) < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut worker = worker.spawn().unwrap();
    drop(provider.worker);
    let client_fd = tunnel.platform.as_raw_fd();
    // SAFETY: The descriptor belongs to this test and is transmitted to the client.
    let flags = unsafe { libc::fcntl(client_fd, libc::F_GETFD) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(client_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
        0
    );
    let mut client = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "relay_pump::tests::separate_broker_initializes_real_mock",
            "--ignored",
            "--nocapture",
        ])
        .env(CLIENT, client_fd.to_string())
        .spawn()
        .unwrap();
    drop(tunnel.platform);
    assert!(client.wait().unwrap().success());
    assert!(broker.wait().unwrap().success());
    // The broker closes the channel after a test check.
    assert!(worker.wait().unwrap().success());
}

fn run_handshake_client(tunnel: std::os::fd::OwnedFd) {
    use carosign_linux_ipc::provider::tunnel::TunnelClient;
    use carosign_protocol::carosign::provider::v1::{Feature, Initialize};
    use std::time::Duration;
    let mut client = TunnelClient::connect(
        tunnel,
        [1; 16],
        7,
        [3; 16],
        [4; 16],
        Duration::from_secs(10),
    )
    .unwrap();
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(10))
            .unwrap()
            .payload,
        Some(Payload::WorkerHello(_))
    ));
    client
        .command(
            ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                worker_instance_id: vec![1; 16],
                frame_seq: 0,
                command_id: vec![],
                operation_id: vec![],
                device_epoch: 0,
                uid: None,
                payload: Some(Payload::Initialize(Initialize {
                    worker_instance_id: vec![1; 16],
                    selected_minor: carosign_protocol::limits::MINOR_BASELINE,
                    package_id: vec![2; 16],
                    profile_id: vec![3; 16],
                    enabled_features: vec![Feature::BasicOperations as i32],
                })),
            },
            Duration::from_secs(10),
        )
        .unwrap();
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(10))
            .unwrap()
            .payload,
        Some(Payload::Initialized(_))
    ));
    use carosign_protocol::carosign::broker_relay::v1::{Claim, Owner, Scope};
    use carosign_protocol::carosign::common::v1::AuthenticatorRef;
    use carosign_protocol::carosign::provider::v1::BindAuthenticator;
    let authenticator = AuthenticatorRef {
        authenticator_id: vec![5; 16],
        device_epoch: 7,
    };
    let bind_operation = if std::env::var_os("CAROSIGN_RELAY_MISMATCH_TEST").is_some() {
        8
    } else {
        12
    };
    client
        .bind(
            Scope {
                lease_id: vec![6; 16],
                owner: Some(Owner {
                    operation_id: vec![bind_operation; 16],
                    worker_instance: vec![1; 16],
                    authenticator_id: vec![5; 16],
                    device_epoch: 7,
                }),
                claims: vec![Claim {
                    resource_id: vec![9; 16],
                    mode: 1,
                }],
            },
            vec![],
            ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                worker_instance_id: vec![1; 16],
                frame_seq: 0,
                command_id: vec![10; 16],
                operation_id: vec![bind_operation; 16],
                device_epoch: 7,
                uid: None,
                payload: Some(Payload::BindAuthenticator(BindAuthenticator {
                    authenticator: Some(authenticator.clone()),
                    profile_id: vec![3; 16],
                    lease_id: vec![6; 16],
                    fd_slots: vec![],
                })),
            },
            Duration::from_secs(10),
        )
        .unwrap();
    let ready = client.receive_event(Duration::from_secs(10)).unwrap();
    assert!(
        matches!(ready.payload, Some(Payload::AuthenticatorReady(ref ready)) if ready.authenticator.as_ref() == Some(&authenticator))
    );
    use carosign_adapters::provider::ProviderChannel;
    use carosign_protocol::carosign::common::v1::{EnrollmentRef, SubjectRef, VerificationPurpose};
    use carosign_protocol::carosign::provider::v1::{
        EnrollmentBinding, EnrollmentBindingKind, StartVerification, VerificationStatus,
    };
    let channel = carosign_linux_ipc::provider::tunnel::TunnelProviderChannel::new(
        client,
        Duration::from_secs(10),
    );
    let verification = channel.send(ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![11; 16],
        operation_id: vec![12; 16],
        device_epoch: 7,
        uid: Some(1000),
        payload: Some(Payload::StartVerification(StartVerification {
            authenticator: Some(authenticator),
            subject: Some(SubjectRef {
                authority_id: vec![11; 16],
                subject_id: vec![12; 16],
            }),
            purpose: VerificationPurpose::PurposeSessionLogin as i32,
            challenge_ref: vec![13; 16],
            recipient_ref: vec![14; 16],
            permitted_enrollments: vec![EnrollmentBinding {
                enrollment: Some(EnrollmentRef {
                    enrollment_id: vec![15; 16],
                    authenticator_id: vec![5; 16],
                    generation: 1,
                }),
                kind: EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: vec![21; 16],
                format_id: String::new(),
                format_version: 0,
            }],
        })),
    });
    if std::env::var_os("CAROSIGN_RELAY_MISMATCH_TEST").is_some() {
        assert!(verification.is_err());
        return;
    }
    verification.unwrap();
    assert!(
        matches!(channel.receive().unwrap().payload, Some(Payload::CommandAck(ref ack)) if ack.accepted)
    );
    if std::env::var_os("CAROSIGN_RELAY_CANCEL_TEST").is_some() {
        use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
        use carosign_core::ids::{
            AuthenticatorId, LeaseId, OperationId, ResourceId, WorkerInstanceId,
        };
        use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
        use carosign_runtime::ports::{ResourceLease, ResourceOwner};

        let lease = ResourceLease::issued_for_fixture(
            LeaseId::from_bytes([6; 16]).unwrap(),
            ResourceOwner {
                operation: OperationId::from_bytes([12; 16]).unwrap(),
                worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
                authenticator: AuthenticatorRef {
                    id: AuthenticatorId::from_bytes([5; 16]).unwrap(),
                    device_epoch: DeviceEpoch::new(7).unwrap(),
                },
            },
            ResourceSet::new(vec![ResourceClaim {
                id: ResourceId::from_bytes([9; 16]).unwrap(),
                mode: AccessMode::Exclusive,
            }])
            .unwrap(),
        );
        use carosign_engine::{EffectReceipt, EffectSpec, PlannedEffect, TransitionIndex};
        use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner, Execution};

        let bridge = carosign_adapters::provider::ProviderBridge::new(channel, [1; 16], 7);
        let ids = CancelIds;
        let mutation = UnusedMutation;
        let mut runner = EffectRunner::new(&bridge, &mutation, DispatchPlans::default())
            .with_scoped_cancel(&lease, &ids);
        let effect = PlannedEffect::at(EffectSpec::RequestCancel, TransitionIndex::new(1), 0);
        assert_eq!(
            futures_executor::block_on(runner.execute(effect, lease.owner().operation)),
            Execution::Receipt(EffectReceipt::CancelDispatched)
        );
        assert_eq!(
            futures_executor::block_on(runner.execute(effect, lease.owner().operation)),
            Execution::Noop
        );
        let channel = bridge.into_channel();
        assert!(
            matches!(channel.receive().unwrap().payload, Some(Payload::CancelReceipt(ref receipt)) if receipt.status == carosign_protocol::carosign::provider::v1::CancelStatus::CancelRequested as i32)
        );
        assert!(
            matches!(channel.receive().unwrap().payload, Some(Payload::VerificationOutcome(ref result)) if result.status == VerificationStatus::VerifyCancelled as i32 && result.evidence.is_none())
        );
        return;
    }
    let result = channel.receive().unwrap();
    let Some(Payload::VerificationOutcome(result)) = result.payload else {
        panic!("ожидался итог проверки")
    };
    if std::env::var_os("CAROSIGN_RELAY_MATCHED_TEST").is_some() {
        assert_eq!(result.status, VerificationStatus::VerifyMatched as i32);
        assert!(
            matches!(result.evidence, Some(carosign_protocol::carosign::provider::v1::verification_outcome::Evidence::OpaqueToken(ref bytes)) if bytes == &[42; 32])
        );
    } else {
        assert_eq!(
            result.status,
            VerificationStatus::VerifyNoUsableSample as i32
        );
    }
}

#[test]
fn separate_test_broker_acknowledges_only_after_provider_send() {
    use carosign_linux_ipc::provider::tunnel::TunnelClient;
    use carosign_protocol::carosign::broker_relay::v1::{Event, Frame, Hello, frame::Body};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::time::{Duration, Instant};

    const TUNNEL_FD: &str = "CAROSIGN_RELAY_TEST_TUNNEL_FD";
    const PROVIDER_FD: &str = "CAROSIGN_RELAY_TEST_PROVIDER_FD";
    if let (Ok(tunnel), Ok(provider)) = (std::env::var(TUNNEL_FD), std::env::var(PROVIDER_FD)) {
        // SAFETY: Only the test parent transmits these descriptors.
        let tunnel =
            PacketSocket::new(unsafe { OwnedFd::from_raw_fd(tunnel.parse().unwrap()) }).unwrap();
        let provider =
            PacketSocket::new(unsafe { OwnedFd::from_raw_fd(provider.parse().unwrap()) }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let incoming = loop {
            match tunnel.receive() {
                Ok(packet) => break packet,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                Err(error) => panic!("получение Hello: {error:?}"),
            }
        };
        let hello = Frame::decode(incoming.as_slice()).unwrap();
        assert!(matches!(hello.body, Some(Body::Hello(_))));
        tunnel
            .send(
                &Frame {
                    major: 1,
                    minor: 0,
                    seq: 1,
                    worker_instance: vec![1; 16],
                    device_epoch: 7,
                    body: Some(Body::Hello(Hello {
                        broker_instance: vec![3; 16],
                        boot_id: vec![4; 16],
                    })),
                }
                .encode_to_vec(),
            )
            .unwrap();
        let mut pump = RelayPump::new(provider, tunnel);
        let mut sequence = 1;
        let mut gate = crate::relay::ShutdownGate::new([1; 16], 7, [3; 16]).unwrap();
        loop {
            match pump.receive_correlated_command([1; 16], 7, sequence, |raw| {
                gate.accept(raw).map_err(|_| ())
            }) {
                Ok(ReceiveRequest::Queued { .. }) => break,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                other => panic!("получение команды: {other:?}"),
            }
        }
        assert!(pump.send_sequenced_command(true, &mut sequence).unwrap());
        let mut tunnel_sequence = 2;
        assert!(
            pump.queue_sent_result([1; 16], 7, &mut tunnel_sequence)
                .unwrap()
        );
        assert!(pump.send_event().unwrap());
        loop {
            match pump.receive_event(|raw| {
                let provider = ProviderFrame::decode(raw).map_err(|_| ())?;
                // The Fixture allows only the canonical first greeting.
                if provider.encode_to_vec() != raw
                    || provider.frame_seq != 1
                    || !matches!(provider.payload, Some(Payload::WorkerHello(_)))
                {
                    return Err(());
                }
                Ok(Frame {
                    major: 1,
                    minor: 0,
                    seq: 3,
                    worker_instance: vec![1; 16],
                    device_epoch: 7,
                    body: Some(Body::Event(Event {
                        frame: Some(provider),
                    })),
                }
                .encode_to_vec())
            }) {
                Ok(true) => break,
                Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                other => panic!("получение события: {other:?}"),
            }
        }
        assert!(pump.send_event().unwrap());
        return;
    }

    let tunnel = socketpair_seqpacket().unwrap();
    let provider = socketpair_seqpacket().unwrap();
    for fd in [tunnel.worker.as_raw_fd(), provider.platform.as_raw_fd()] {
        // SAFETY: The flags are changed only for the FD test.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "relay_pump::tests::separate_test_broker_acknowledges_only_after_provider_send",
            "--nocapture",
        ])
        .env(TUNNEL_FD, tunnel.worker.as_raw_fd().to_string())
        .env(PROVIDER_FD, provider.platform.as_raw_fd().to_string())
        .spawn()
        .unwrap();
    drop(tunnel.worker);
    drop(provider.platform);
    let mut client = TunnelClient::connect(
        tunnel.platform,
        [1; 16],
        7,
        [3; 16],
        [4; 16],
        Duration::from_secs(5),
    )
    .unwrap();
    client
        .command(
            ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                worker_instance_id: vec![1; 16],
                frame_seq: 0,
                command_id: vec![2; 16],
                operation_id: vec![],
                device_epoch: 7,
                uid: None,
                payload: Some(Payload::Shutdown(Shutdown::default())),
            },
            Duration::from_secs(5),
        )
        .unwrap();
    let worker = PacketSocket::new(provider.worker).unwrap();
    let delivered = ProviderFrame::decode(worker.receive().unwrap().as_slice()).unwrap();
    assert_eq!(delivered.frame_seq, 1);
    assert!(matches!(delivered.payload, Some(Payload::Shutdown(_))));
    worker
        .send(
            &ProviderFrame {
                major: 1,
                minor: 3,
                frame_seq: 1,
                payload: Some(Payload::WorkerHello(
                    carosign_protocol::carosign::provider::v1::WorkerHello {
                        major: 1,
                        min_minor: 3,
                        max_minor: 3,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }
            .encode_to_vec(),
        )
        .unwrap();
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(5))
            .unwrap()
            .payload,
        Some(Payload::WorkerHello(_))
    ));
    assert!(child.wait().unwrap().success());
}

#[test]
fn encoded_daemon_commands_reach_provider_through_relay() {
    use carosign_linux_ipc::provider::tunnel::TunnelCommandEncoder;
    use carosign_protocol::carosign::broker_relay::v1::{Frame, frame::Body};

    // The trial links the real demon encoder to the broker's pump.
    // The front checker here does not confirm trust or strict wire.
    let (mut pump, worker, daemon) = sockets();
    let mut encoder = TunnelCommandEncoder::after_hello([1; 16], 7, [3; 16]).unwrap();
    let mut provider_seq = 1;
    for tunnel_seq in 2..=3 {
        let command = ProviderFrame {
            major: 1,
            minor: 3,
            worker_instance_id: vec![1; 16],
            frame_seq: 0,
            command_id: vec![2; 16],
            operation_id: vec![],
            device_epoch: 7,
            uid: None,
            payload: Some(Payload::Shutdown(Shutdown::default())),
        };
        let (packet, request_id) = encoder.command(command).unwrap();
        daemon.send(&packet).unwrap();
        pump.receive_sequenced_command([1; 16], 7, &mut provider_seq, |raw| {
            let tunnel = Frame::decode(raw).map_err(|_| ())?;
            if tunnel.major != 1
                || tunnel.minor != 0
                || tunnel.seq != tunnel_seq
                || tunnel.worker_instance != [1; 16]
                || tunnel.device_epoch != 7
            {
                return Err(());
            }
            let Some(Body::Command(command)) = tunnel.body else {
                return Err(());
            };
            if command.request_id != request_id {
                return Err(());
            }
            command.frame.ok_or(())
        })
        .unwrap();
        assert!(
            pump.send_sequenced_command(true, &mut provider_seq)
                .unwrap()
        );
        let delivered = ProviderFrame::decode(worker.receive().unwrap().as_slice()).unwrap();
        assert_eq!(delivered.frame_seq, tunnel_seq - 1);
        assert!(matches!(delivered.payload, Some(Payload::Shutdown(_))));
    }
}

#[test]
fn initialize_preserves_pre_bind_epoch_through_relay() {
    use carosign_linux_ipc::provider::tunnel::TunnelCommandEncoder;
    use carosign_protocol::carosign::broker_relay::v1::{Frame, frame::Body};
    use carosign_protocol::carosign::provider::v1::Initialize;

    let (mut pump, worker, daemon) = sockets();
    let mut encoder = TunnelCommandEncoder::after_hello([1; 16], 7, [3; 16]).unwrap();
    let command = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![],
        operation_id: vec![],
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::Initialize(Initialize {
            worker_instance_id: vec![1; 16],
            selected_minor: 3,
            package_id: vec![2; 16],
            profile_id: vec![3; 16],
            enabled_features: vec![],
        })),
    };
    let (packet, _) = encoder.command(command).unwrap();
    daemon.send(&packet).unwrap();
    let mut sequence = 1;
    assert!(
        pump.receive_sequenced_command([1; 16], 7, &mut sequence, |raw| {
            match Frame::decode(raw).map_err(|_| ())?.body {
                Some(Body::Command(command)) => command.frame.ok_or(()),
                _ => Err(()),
            }
        })
        .unwrap()
    );
    assert!(pump.send_sequenced_command(true, &mut sequence).unwrap());
    let delivered = ProviderFrame::decode(worker.receive().unwrap().as_slice()).unwrap();
    assert_eq!(delivered.device_epoch, 0);
    assert_eq!(delivered.frame_seq, 1);
    assert!(matches!(delivered.payload, Some(Payload::Initialize(_))));
}

#[test]
fn assigns_provider_sequence_at_the_sender_and_rejects_external_number() {
    let (mut pump, worker, daemon) = sockets();
    let mut seq = 1;
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![],
        device_epoch: 7,
        uid: None,
        payload: Some(Payload::Shutdown(Shutdown::default())),
    };
    for expected in 1..=2 {
        daemon.send(b"checked-tunnel").unwrap();
        pump.receive_sequenced_command([1; 16], 7, &mut seq, |raw| {
            assert_eq!(raw, b"checked-tunnel");
            Ok(frame.clone())
        })
        .unwrap();
        assert!(pump.send_sequenced_command(true, &mut seq).unwrap());
        let sent = ProviderFrame::decode(worker.receive().unwrap().as_slice()).unwrap();
        assert_eq!(sent.frame_seq, expected);
    }
    assert_eq!(seq, 3);
    daemon.send(b"checked-tunnel").unwrap();
    let mut external = frame;
    external.frame_seq = 55;
    assert!(matches!(
        pump.receive_sequenced_command([1; 16], 7, &mut seq, |_| Ok(external)),
        Err(TransportError::Invalid)
    ));
    assert_eq!(seq, 3);
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
}

#[test]
fn control_overtakes_unselected_command_and_sequence_is_retained_on_eagain() {
    let (mut pump, worker, daemon) = sockets();
    let mut seq = 1;
    let command = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![],
        device_epoch: 7,
        uid: None,
        payload: Some(Payload::Shutdown(Shutdown::default())),
    };
    daemon.send(b"checked").unwrap();
    pump.receive_sequenced_command([1; 16], 7, &mut seq, |_| Ok(command.clone()))
        .unwrap();
    assert_eq!(seq, 1, "постановка не занимает номер");
    let control = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![3; 16],
        operation_id: vec![4; 16],
        device_epoch: 7,
        uid: None,
        payload: Some(Payload::RequestCancel(
            carosign_protocol::carosign::provider::v1::RequestCancel {
                target_operation_id: vec![4; 16],
            },
        )),
    };
    pump.sequenced_control(control).unwrap();
    while pump.provider_for_test().send(&[7; 1024]).is_ok() {}
    assert!(matches!(
        pump.send_sequenced_command(true, &mut seq),
        Err(TransportError::WouldBlock)
    ));
    assert_eq!(seq, 2);
    while worker.receive().is_ok() {}
    assert!(pump.send_sequenced_command(true, &mut seq).unwrap());
    let first = ProviderFrame::decode(worker.receive().unwrap().as_slice()).unwrap();
    assert_eq!(first.frame_seq, 1);
    assert!(matches!(first.payload, Some(Payload::RequestCancel(_))));
    assert_eq!(seq, 2, "повтор не занимает новый номер");
    assert!(pump.send_sequenced_command(true, &mut seq).unwrap());
    let second = ProviderFrame::decode(worker.receive().unwrap().as_slice()).unwrap();
    assert_eq!(second.frame_seq, 2);
    assert!(matches!(second.payload, Some(Payload::Shutdown(_))));
    assert_eq!(seq, 3);
}

#[test]
fn sequenced_blocked_packet_is_not_sent_after_revoke() {
    let (mut pump, worker, daemon) = sockets();
    let mut seq = 1;
    let command = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![],
        device_epoch: 7,
        uid: None,
        payload: Some(Payload::Shutdown(Shutdown::default())),
    };
    daemon.send(b"checked").unwrap();
    pump.receive_sequenced_command([1; 16], 7, &mut seq, |_| Ok(command))
        .unwrap();
    while pump.provider_for_test().send(&[7; 1024]).is_ok() {}
    assert!(matches!(
        pump.send_sequenced_command(true, &mut seq),
        Err(TransportError::WouldBlock)
    ));
    pump.revoke();
    while worker.receive().is_ok() {}
    assert!(matches!(
        pump.send_sequenced_command(true, &mut seq),
        Err(TransportError::Closed)
    ));
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
}

#[test]
fn receives_validated_packets_from_both_real_sockets() {
    let (mut pump, worker, daemon) = sockets();
    daemon.send(b"checked-command").unwrap();
    assert!(
        pump.receive_command(|raw| {
            (raw == b"checked-command")
                .then(|| b"provider-command".to_vec())
                .ok_or(())
        })
        .unwrap()
    );
    assert!(pump.send_command(true).unwrap());
    assert_eq!(worker.receive().unwrap(), b"provider-command");
    worker.send(b"checked-event").unwrap();
    assert!(
        pump.receive_event(|raw| {
            (raw == b"checked-event")
                .then(|| b"tunnel-event".to_vec())
                .ok_or(())
        })
        .unwrap()
    );
    assert!(pump.send_event().unwrap());
    assert_eq!(daemon.receive().unwrap(), b"tunnel-event");
}

#[test]
fn rejected_packet_and_peer_loss_close_both_sockets() {
    let (mut pump, worker, daemon) = sockets();
    daemon.send(b"invalid").unwrap();
    assert!(matches!(
        pump.receive_command(|_| Err(())),
        Err(TransportError::Invalid)
    ));
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
    assert!(matches!(daemon.receive(), Err(TransportError::Closed)));

    let (mut pump, worker, daemon) = sockets();
    drop(worker);
    // Parallel fork can briefly hold a copy of peer to exec.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let result = loop {
        match pump.receive_event(|_| unreachable!()) {
            Err(TransportError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            result => break result,
        }
    };
    assert!(matches!(result, Err(TransportError::Closed)));
    assert!(matches!(daemon.receive(), Err(TransportError::Closed)));
}

#[test]
fn exchanges_ordered_packets_and_revokes_before_next_send() {
    let (mut pump, worker, daemon) = sockets();
    pump.command(b"first".to_vec()).unwrap();
    pump.command(b"second".to_vec()).unwrap();
    assert!(pump.send_command(true).unwrap());
    assert_eq!(worker.receive().unwrap(), b"first");
    pump.event(b"event".to_vec()).unwrap();
    assert!(pump.send_event().unwrap());
    assert_eq!(daemon.receive().unwrap(), b"event");
    pump.revoke();
    assert!(matches!(
        pump.send_command(true),
        Err(TransportError::Closed)
    ));
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
}

#[test]
fn refusal_before_send_closes_both_sides() {
    let (mut pump, worker, daemon) = sockets();
    pump.command(b"command".to_vec()).unwrap();
    assert!(matches!(
        pump.send_command(false),
        Err(TransportError::Invalid)
    ));
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
    assert!(matches!(daemon.receive(), Err(TransportError::Closed)));
}

#[test]
fn event_overflow_terminates_without_silent_drop() {
    let (mut pump, worker, daemon) = sockets();
    for _ in 0..64 {
        pump.event(vec![1]).unwrap();
    }
    assert_eq!(pump.event(vec![2]), Err(QueueError::Closed));
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
    assert!(matches!(daemon.receive(), Err(TransportError::Closed)));
}

#[test]
fn blocked_command_counts_against_limit_and_revoke_prevents_retry() {
    let (mut pump, worker, _daemon) = sockets();
    loop {
        match pump.provider_for_test().send(&[7; 1024]) {
            Ok(()) => {}
            Err(TransportError::WouldBlock) => break,
            Err(error) => panic!("ошибка заполнения: {error:?}"),
        }
    }
    pump.command(b"pending".to_vec()).unwrap();
    assert!(matches!(
        pump.send_command(true),
        Err(TransportError::WouldBlock)
    ));
    for _ in 0..63 {
        pump.command(vec![1]).unwrap();
    }
    assert_eq!(pump.command(vec![2]), Err(QueueError::Busy));
    pump.revoke();
    while worker.receive().is_ok() {}
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
}

#[test]
fn blocked_event_counts_against_limit() {
    let (mut pump, _worker, _daemon) = sockets();
    loop {
        match pump.tunnel_for_test().send(&[7; 1024]) {
            Ok(()) => {}
            Err(TransportError::WouldBlock) => break,
            Err(error) => panic!("ошибка заполнения: {error:?}"),
        }
    }
    pump.event(b"terminal".to_vec()).unwrap();
    assert!(matches!(pump.send_event(), Err(TransportError::WouldBlock)));
    for _ in 0..63 {
        pump.event(vec![1]).unwrap();
    }
    assert_eq!(pump.event(vec![2]), Err(QueueError::Closed));
}
