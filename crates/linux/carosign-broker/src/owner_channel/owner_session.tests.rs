use super::*;
use carosign_protocol::carosign::{broker_relay::v1 as relay, provider::v1::FdRole};
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SocketFlags, SocketType, sendmsg,
    socketpair,
};
use std::io::IoSlice;
use std::os::fd::{AsFd, AsRawFd};

fn request() -> wire::Prepare {
    wire::Prepare {
        request_id: vec![1; 16],
        boot_id: vec![2; 16],
        scope: Some(relay::Scope {
            lease_id: vec![3; 16],
            owner: Some(relay::Owner {
                operation_id: vec![4; 16],
                worker_instance: vec![5; 16],
                authenticator_id: vec![6; 16],
                device_epoch: 1,
            }),
            claims: vec![relay::Claim {
                resource_id: vec![7; 16],
                mode: relay::AccessMode::Exclusive as i32,
            }],
        }),
        slots: vec![relay::Slot {
            resource_id: vec![7; 16],
            role: FdRole::Device as i32,
        }],
    }
}

#[test]
fn owner_dispatch_checks_receipt_and_cannot_repeat() {
    use carosign_protocol::carosign::provider::v1::ProviderFrame;

    for matching in [true, false] {
        let (client, owner) =
            socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, SocketFlags::CLOEXEC, None)
                .unwrap();
        let mut session = OwnerSession {
            fd: client,
            prepare: request(),
            requests: Default::default(),
            terminal: false,
            dispatched: false,
        };
        let frame = ProviderFrame { command_id: vec![8; 16], ..Default::default() };
        let prepare = session.prepare.clone();
        let expected = frame.clone();
        let worker = std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            let (size, _) = rustix::net::recv(&owner, &mut bytes, RecvFlags::empty()).unwrap();
            let control = wire::OwnerControl::decode(&bytes[..size]).unwrap();
            assert_eq!(control.boot_id, prepare.boot_id);
            assert_eq!(control.scope, prepare.scope);
            assert_eq!(control.dispatch, Some(expected.clone()));
            let response = wire::DispatchObservation {
                request_id: control.request_id,
                command_id: if matching { expected.command_id } else { vec![9; 16] },
                confirmed: true,
            };
            send(&owner, &response.encode_to_vec(), SendFlags::NOSIGNAL).unwrap();
        });
        assert_eq!(session.dispatch([10; 16], &frame, Duration::from_secs(2)).is_ok(), matching);
        assert!(session.dispatch([11; 16], &frame, Duration::from_secs(2)).is_err());
        worker.join().unwrap();
    }
}

#[test]
fn transfers_rights_only_after_matching_response_and_keeps_channel() {
    let (client, owner) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let expected = request();
    let thread = std::thread::spawn(move || {
        let mut data = [0u8; 4096];
        let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
        let received = wire::Prepare::decode(&data[..size]).unwrap();
        let response = wire::Response {
            request_id: received.request_id,
            boot_id: received.boot_id,
            result: Some(wire::response::Result::Prepared(wire::Prepared {
                scope: received.scope,
                slots: received.slots,
            })),
        }
        .encode_to_vec();
        let file = std::fs::File::open("/dev/null").unwrap();
        let borrowed = [file.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        assert!(control.push(SendAncillaryMessage::ScmRights(&borrowed)));
        sendmsg(
            &owner,
            &[IoSlice::new(&response)],
            &mut control,
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
        assert_eq!(size, 0);
    });
    let reply = prepare_once(client, &expected, Duration::from_secs(3)).unwrap();
    match reply {
        Reply::Prepared {
            channel,
            descriptors,
        } => {
            assert_eq!(descriptors.len(), 1);
            // SAFETY: F_GETFD He reads the flags of a living descriptor.
            assert_ne!(
                unsafe { libc::fcntl(descriptors[0].as_raw_fd(), libc::F_GETFD) }
                    & libc::FD_CLOEXEC,
                0
            );
            drop(channel);
        }
        Reply::Refused(_) => panic!("ожидался FD"),
    }
    thread.join().unwrap();
}

#[test]
fn lost_reply_is_timed_out_not_success() {
    let (client, owner) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let result = prepare_once(client, &request(), Duration::from_millis(20));
    assert_eq!(result.err().unwrap().kind(), io::ErrorKind::TimedOut);
    drop(owner);
}

#[test]
fn rejects_stream_socket_before_sending() {
    let (client, owner) = socketpair(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let result = prepare_once(client, &request(), Duration::from_secs(1));
    assert_eq!(result.err().unwrap().kind(), io::ErrorKind::InvalidData);
    let mut data = [0; 1];
    let (bytes, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
    assert_eq!(bytes, 0);
}

#[test]
fn quiescence_stages_are_scoped_and_terminal_without_hardware_proof() {
    let (client, owner) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let expected = request();
    let thread = std::thread::spawn(move || {
        let mut data = [0u8; 4096];
        let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
        let received = wire::Prepare::decode(&data[..size]).unwrap();
        let response = wire::Response {
            request_id: received.request_id.clone(),
            boot_id: received.boot_id.clone(),
            result: Some(wire::response::Result::Prepared(wire::Prepared {
                scope: received.scope.clone(),
                slots: received.slots.clone(),
            })),
        }
        .encode_to_vec();
        let file = std::fs::File::open("/dev/null").unwrap();
        let borrowed = [file.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        assert!(control.push(SendAncillaryMessage::ScmRights(&borrowed)));
        sendmsg(
            &owner,
            &[IoSlice::new(&response)],
            &mut control,
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        for stage in [1, 2] {
            let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
            let inspect = wire::InspectQuiescence::decode(&data[..size]).unwrap();
            assert_eq!(inspect.scope, received.scope);
            assert_eq!(inspect.boot_id, received.boot_id);
            let observation = wire::QuiescenceObservation {
                request_id: inspect.request_id,
                boot_id: inspect.boot_id,
                scope: inspect.scope,
                stage,
            }
            .encode_to_vec();
            send(&owner, &observation, SendFlags::NOSIGNAL).unwrap();
        }
    });
    let Reply::Prepared {
        mut channel,
        descriptors,
    } = prepare_once(client, &expected, Duration::from_secs(3)).unwrap()
    else {
        panic!("нет ресурсов")
    };
    assert_eq!(
        channel
            .inspect_quiescence([9; 16], Duration::from_secs(3))
            .unwrap(),
        Quiescence::Draining
    );
    assert_eq!(
        channel
            .inspect_quiescence([10; 16], Duration::from_secs(3))
            .unwrap(),
        Quiescence::HardwareUnconfirmed
    );
    assert!(
        channel
            .inspect_quiescence([11; 16], Duration::from_secs(3))
            .is_err()
    );
    drop(descriptors);
    drop(channel);
    thread.join().unwrap();
}

#[test]
fn repeated_quiescence_id_permanently_closes_request_path() {
    let (client, owner) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let prepared = request();
    let thread = std::thread::spawn(move || {
        let mut data = [0u8; 4096];
        let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
        let inspect = wire::InspectQuiescence::decode(&data[..size]).unwrap();
        let observation = wire::QuiescenceObservation {
            request_id: inspect.request_id,
            boot_id: inspect.boot_id,
            scope: inspect.scope,
            stage: 1,
        }
        .encode_to_vec();
        send(&owner, &observation, SendFlags::NOSIGNAL).unwrap();
        let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
        assert_eq!(size, 0);
    });
    let mut channel = OwnerSession {
        fd: client,
        prepare: prepared,
        requests: Default::default(),
        terminal: false,
        dispatched: false,
    };
    assert_eq!(
        channel
            .inspect_quiescence([9; 16], Duration::from_secs(3))
            .unwrap(),
        Quiescence::Draining
    );
    assert!(
        channel
            .inspect_quiescence([9; 16], Duration::from_secs(3))
            .is_err()
    );
    assert!(
        channel
            .inspect_quiescence([10; 16], Duration::from_secs(3))
            .is_err()
    );
    drop(channel);
    thread.join().unwrap();
}

#[test]
fn revoked_descriptors_are_scoped_observation_not_global_quiescence() {
    let (client, owner) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let prepared = request();
    let expected_scope = prepared.scope.clone();
    let thread = std::thread::spawn(move || {
        let mut data = [0u8; 4096];
        for stage in [1, 4] {
            let (size, _) = rustix::net::recv(&owner, &mut data[..], RecvFlags::empty()).unwrap();
            let inspect = wire::InspectQuiescence::decode(&data[..size]).unwrap();
            assert_eq!(inspect.scope, expected_scope);
            let observation = wire::QuiescenceObservation {
                request_id: inspect.request_id,
                boot_id: inspect.boot_id,
                scope: inspect.scope,
                stage,
            }
            .encode_to_vec();
            send(&owner, &observation, SendFlags::NOSIGNAL).unwrap();
        }
    });
    let mut session = OwnerSession {
        fd: client,
        prepare: prepared,
        requests: Default::default(),
        terminal: false,
        dispatched: false,
    };
    assert_eq!(
        session
            .inspect_quiescence([9; 16], Duration::from_secs(3))
            .unwrap(),
        Quiescence::Draining
    );
    assert_eq!(
        session
            .inspect_quiescence([10; 16], Duration::from_secs(3))
            .unwrap(),
        Quiescence::LeaseRevoked
    );
    assert!(
        session
            .inspect_quiescence([11; 16], Duration::from_secs(3))
            .is_err()
    );
    thread.join().unwrap();
}
