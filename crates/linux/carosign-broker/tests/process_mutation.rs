//! Testing the disposable command boundary on a real mediator process.
use carosign_broker::transport::{PacketSocket, TransportError};
use carosign_linux::sys::socketpair_seqpacket;
use carosign_protocol::carosign::{
    broker_relay::v1 as relay, common::v1 as common, provider::v1 as provider,
};
use prost::Message;
use std::{
    io::Read,
    os::fd::{AsRawFd, FromRawFd},
    time::{Duration, Instant},
};

fn recv(socket: &PacketSocket) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match socket.receive() {
            Ok(bytes) => return bytes,
            Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                std::thread::yield_now()
            }
            other => panic!("ожидался пакет: {other:?}"),
        }
    }
}

fn request(seq: u64) -> Vec<u8> {
    let mut id = [3u8; 16];
    for (dst, byte) in id[8..].iter_mut().zip(seq.to_be_bytes()) {
        *dst ^= byte;
    }
    id.to_vec()
}

fn envelope(seq: u64, body: relay::frame::Body) -> Vec<u8> {
    relay::Frame {
        major: 1,
        minor: 0,
        seq,
        worker_instance: vec![1; 16],
        device_epoch: 7,
        body: Some(body),
    }
    .encode_to_vec()
}

fn frame(seq: u64, payload: provider::provider_frame::Payload) -> provider::ProviderFrame {
    provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: vec![1; 16],
        frame_seq: seq,
        command_id: vec![9; 16],
        operation_id: vec![8; 16],
        device_epoch: 7,
        uid: matches!(
            &payload,
            provider::provider_frame::Payload::StartVerification(_)
                | provider::provider_frame::Payload::StartEnrollment(_)
                | provider::provider_frame::Payload::ContinueEnrollment(_)
                | provider::provider_frame::Payload::StartRemoval(_)
                | provider::provider_frame::Payload::PrepareEnrollmentChallenge(_)
        )
        .then_some(1000),
        payload: Some(payload),
    }
}

#[test]
fn process_forwards_one_bound_mutation_and_rejects_replay() {
    process_forwards_bound_mutation(false, false, false, false, false, false, false);
}

#[test]
fn process_forwards_one_authorized_continuation_and_rejects_replay() {
    process_forwards_bound_mutation(false, false, false, false, false, false, true);
}

#[test]
fn process_forwards_complete_two_resource_binding() {
    process_forwards_bound_mutation(true, false, false, false, false, false, false);
}

#[test]
fn process_forwards_owner_resources_after_scope_validation() {
    process_forwards_bound_mutation(true, false, false, false, true, false, false);
}

#[test]
fn process_forwards_two_owned_devices_and_sealed_config() {
    process_forwards_bound_mutation(true, false, false, false, true, true, false);
}

#[test]
fn process_rejects_partial_open_without_transferring_any_descriptor() {
    process_forwards_bound_mutation(true, true, false, false, false, false, false);
}

#[test]
fn process_rejects_reordered_resource_slots() {
    process_forwards_bound_mutation(true, false, true, false, false, false, false);
}

#[test]
fn process_rejects_reordered_provider_indices() {
    process_forwards_bound_mutation(true, false, false, true, false, false, false);
}

#[test]
fn configured_resources_reject_duplicates_and_more_than_eight() {
    use carosign_broker::process::{ConfiguredResource, Context, run_with_resources};
    for ids in [vec![1, 1], (1..=9).collect()] {
        let tunnel = socketpair_seqpacket().unwrap();
        let provider = socketpair_seqpacket().unwrap();
        let resources = ids
            .into_iter()
            .map(|id| ConfiguredResource {
                path: "/dev/null".into(),
                resource_id: [id; 16],
                role: provider::FdRole::Config as i32,
            })
            .collect();
        let context = Context {
            worker: [1; 16],
            epoch: 7,
            broker: [3; 16],
            boot: [4; 16],
        };
        assert!(matches!(
            run_with_resources(tunnel.worker, provider.platform, context, resources),
            Err(TransportError::Invalid)
        ));
    }
}

fn process_forwards_bound_mutation(
    two_resources: bool,
    reject_second: bool,
    reordered: bool,
    wrong_index: bool,
    use_owner: bool,
    mixed: bool,
    continuation: bool,
) {
    use provider::provider_frame::Payload;
    use relay::frame::Body;
    let tunnel = socketpair_seqpacket().unwrap();
    let provider = socketpair_seqpacket().unwrap();
    let owner = use_owner.then(|| socketpair_seqpacket().unwrap());
    let control = use_owner.then(|| socketpair_seqpacket().unwrap());
    // makepkg can unpack the source code into a common /tmp, which the broker must
    // reject.
    use std::os::unix::fs::DirBuilderExt;
    let directory =
        std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(format!(
            ".carosign-broker-test-{}-{two_resources}-{reject_second}-{reordered}-{wrong_index}-{use_owner}-{mixed}-{continuation}",
            std::process::id()
        ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let path = directory.join("resource");
    std::fs::write(&path, b"test resource").unwrap();
    let second_path = directory.join("second");
    if two_resources && !reject_second {
        std::fs::write(&second_path, b"second resource").unwrap();
    }
    for fd in [tunnel.worker.as_raw_fd(), provider.platform.as_raw_fd()] {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    if let Some(pair) = &owner {
        let fd = pair.worker.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    if let Some(pair) = &control {
        let fd = pair.worker.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_carosign-broker"));
    command.args([
        "--tunnel-fd",
        &tunnel.worker.as_raw_fd().to_string(),
        "--provider-fd",
        &provider.platform.as_raw_fd().to_string(),
        "--worker",
        "01010101010101010101010101010101",
        "--epoch",
        "7",
        "--broker",
        "03030303030303030303030303030303",
        "--boot",
        "04040404040404040404040404040404",
    ]);
    if let Some(pair) = &owner {
        command.args(["--owner-fd", &pair.worker.as_raw_fd().to_string()]);
    }
    if let Some(pair) = &control {
        command.args(["--control-fd", &pair.worker.as_raw_fd().to_string()]);
    }
    let device_one = std::path::PathBuf::from("/dev/null");
    let device_two = std::path::PathBuf::from("/dev/zero");
    command.args([
        "--resource-path",
        if mixed {
            device_one.to_str().unwrap()
        } else {
            path.to_str().unwrap()
        },
        "--resource-id",
        "07070707070707070707070707070707",
        "--resource-role",
        if mixed { "device" } else { "config" },
    ]);
    if two_resources {
        command.args([
            "--resource-path",
            if mixed {
                device_two.to_str().unwrap()
            } else {
                second_path.to_str().unwrap()
            },
            "--resource-id",
            "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
            "--resource-role",
            if mixed { "device" } else { "config" },
        ]);
    }
    if mixed {
        command.args([
            "--resource-path",
            path.to_str().unwrap(),
            "--resource-id",
            "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
            "--resource-role",
            "config",
        ]);
    }
    let mut child = command.spawn().unwrap();
    let owner_thread = owner.map(|pair| {
        use std::os::fd::AsFd;
        let owner_paths = if mixed {
            [device_one, device_two]
        } else {
            [path.clone(), second_path.clone()]
        };
        drop(pair.worker);
        std::thread::spawn(move || {
            use carosign_protocol::carosign::resource_owner::v1 as owner_wire;
            use rustix::net::{
                RecvFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg,
            };
            let mut bytes = [0u8; 4096];
            let (count, _) =
                rustix::net::recv(&pair.platform, &mut bytes[..], RecvFlags::empty()).unwrap();
            let request = owner_wire::Prepare::decode(&bytes[..count]).unwrap();
            if mixed {
                assert_eq!(request.slots.len(), 2);
                assert_eq!(request.scope.as_ref().unwrap().claims.len(), 3);
            }
            let response = owner_wire::Response {
                request_id: request.request_id,
                boot_id: request.boot_id,
                result: Some(owner_wire::response::Result::Prepared(
                    owner_wire::Prepared {
                        scope: request.scope,
                        slots: request.slots,
                    },
                )),
            }
            .encode_to_vec();
            let files: Vec<_> = owner_paths
                .iter()
                .map(|path| {
                    std::fs::OpenOptions::new()
                        .read(true)
                        .write(mixed)
                        .open(path)
                        .unwrap()
                })
                .collect();
            let fds: Vec<_> = files.iter().map(|file| file.as_fd()).collect();
            let mut cmsg_space =
                [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
            let mut control = SendAncillaryBuffer::new(&mut cmsg_space);
            assert!(control.push(SendAncillaryMessage::ScmRights(&fds)));
            sendmsg(
                &pair.platform,
                &[std::io::IoSlice::new(&response)],
                &mut control,
                SendFlags::NOSIGNAL,
            )
            .unwrap();
            let mut previous = Vec::new();
            // 40 intermediate responses exceed the previous limit of 32 and confirm a repeat
            // after the first 320 ms wait.
            for stage in std::iter::repeat_n(owner_wire::QuiescenceStage::QuiescenceDraining, 40)
                .chain(std::iter::once(
                    owner_wire::QuiescenceStage::QuiescenceExclusiveQuiesced,
                ))
            {
                let (count, _) =
                    rustix::net::recv(&pair.platform, &mut bytes[..], RecvFlags::empty()).unwrap();
                let request = owner_wire::InspectQuiescence::decode(&bytes[..count]).unwrap();
                assert_ne!(request.request_id, previous);
                previous = request.request_id.clone();
                let response = owner_wire::QuiescenceObservation {
                    request_id: request.request_id,
                    boot_id: request.boot_id,
                    scope: request.scope,
                    stage: stage as i32,
                }
                .encode_to_vec();
                rustix::net::send(&pair.platform, &response, SendFlags::NOSIGNAL).unwrap();
            }
        })
    });
    drop(tunnel.worker);
    drop(provider.platform);
    let tunnel = PacketSocket::new(tunnel.platform).unwrap();
    let worker_fd = provider.worker.as_raw_fd();
    let worker = PacketSocket::new(provider.worker).unwrap();
    tunnel
        .send(&envelope(
            1,
            Body::Hello(relay::Hello {
                broker_instance: vec![3; 16],
                boot_id: vec![4; 16],
            }),
        ))
        .unwrap();
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::Hello(_))
    ));
    let hello = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        frame_seq: 1,
        payload: Some(Payload::WorkerHello(provider::WorkerHello {
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
            ..Default::default()
        })),
        ..Default::default()
    };
    worker.send(&hello.encode_to_vec()).unwrap();
    let _ = recv(&tunnel);
    let mut init = frame(
        0,
        Payload::Initialize(provider::Initialize {
            worker_instance_id: vec![1; 16],
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: vec![2; 16],
            profile_id: vec![3; 16],
            enabled_features: vec![provider::Feature::BasicOperations as i32],
        }),
    );
    init.operation_id.clear();
    init.command_id.clear();
    init.device_epoch = 0;
    tunnel
        .send(&envelope(
            2,
            Body::Command(relay::Command {
                request_id: request(2),
                frame: Some(init),
                mutation_permit: None,
            }),
        ))
        .unwrap();
    let _ = recv(&worker);
    let _ = recv(&tunnel);
    let mut initialized = frame(
        2,
        Payload::Initialized(provider::Initialized {
            ready_for_binding: true,
        }),
    );
    initialized.operation_id.clear();
    initialized.command_id.clear();
    initialized.device_epoch = 0;
    worker.send(&initialized.encode_to_vec()).unwrap();
    let _ = recv(&tunnel);
    let auth = common::AuthenticatorRef {
        authenticator_id: vec![5; 16],
        device_epoch: 7,
    };
    let mut scope = relay::Scope {
        lease_id: vec![6; 16],
        owner: Some(relay::Owner {
            operation_id: vec![8; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![relay::Claim {
            resource_id: vec![7; 16],
            mode: relay::AccessMode::Exclusive as i32,
        }],
    };
    if two_resources {
        scope.claims.push(relay::Claim {
            resource_id: vec![10; 16],
            mode: relay::AccessMode::Exclusive as i32,
        });
    }
    if mixed {
        scope.claims.push(relay::Claim {
            resource_id: vec![11; 16],
            mode: relay::AccessMode::Exclusive as i32,
        });
    }
    let mut bind = frame(
        0,
        Payload::BindAuthenticator(provider::BindAuthenticator {
            authenticator: Some(auth.clone()),
            profile_id: vec![3; 16],
            lease_id: vec![6; 16],
            fd_slots: vec![provider::FdSlot {
                index: 0,
                role: if mixed {
                    provider::FdRole::Device
                } else {
                    provider::FdRole::Config
                } as i32,
                resource_id: vec![7; 16],
            }],
        }),
    );
    if two_resources {
        let Some(Payload::BindAuthenticator(body)) = &mut bind.payload else {
            unreachable!()
        };
        body.fd_slots.push(provider::FdSlot {
            index: 1,
            role: if mixed {
                provider::FdRole::Device
            } else {
                provider::FdRole::Config
            } as i32,
            resource_id: vec![10; 16],
        });
        if wrong_index {
            body.fd_slots[1].index = 0;
        }
    }
    if mixed {
        let Some(Payload::BindAuthenticator(body)) = &mut bind.payload else {
            unreachable!()
        };
        body.fd_slots.push(provider::FdSlot {
            index: 2,
            role: provider::FdRole::Config as i32,
            resource_id: vec![11; 16],
        });
    }
    let mut slots = vec![relay::Slot {
        role: if mixed {
            provider::FdRole::Device
        } else {
            provider::FdRole::Config
        } as i32,
        resource_id: vec![7; 16],
    }];
    if two_resources {
        slots.push(relay::Slot {
            role: if mixed {
                provider::FdRole::Device
            } else {
                provider::FdRole::Config
            } as i32,
            resource_id: vec![10; 16],
        });
    }
    if mixed {
        slots.push(relay::Slot {
            role: provider::FdRole::Config as i32,
            resource_id: vec![11; 16],
        });
    }
    if reordered {
        slots.swap(0, 1);
    }
    tunnel
        .send(&envelope(
            3,
            Body::Bind(relay::Bind {
                request_id: request(3),
                scope: Some(scope.clone()),
                slots,
                frame: Some(bind),
            }),
        ))
        .unwrap();
    // That's what we're getting. SCM_RIGHTS from a separate mediator process.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = [0u8; 4096];
    let mut ancillary = [0usize; 32];
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = ancillary.as_mut_ptr().cast();
    message.msg_controllen = std::mem::size_of_val(&ancillary);
    loop {
        let count = unsafe { libc::recvmsg(worker_fd, &mut message, 0) };
        if count >= 0 {
            break;
        }
        assert!(
            Instant::now() < deadline
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock
        );
        std::thread::yield_now();
    }
    let header = unsafe { libc::CMSG_FIRSTHDR(&message) };
    if reject_second || reordered || wrong_index {
        assert!(header.is_null(), "частичный набор не должен передаваться");
        assert!(!child.wait().unwrap().success());
        std::fs::remove_file(path).unwrap();
        if reordered || wrong_index {
            std::fs::remove_file(second_path).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
        return;
    }
    assert!(!header.is_null());
    assert_eq!(unsafe { (*header).cmsg_level }, libc::SOL_SOCKET);
    assert_eq!(unsafe { (*header).cmsg_type }, libc::SCM_RIGHTS);
    let fd = unsafe { *(libc::CMSG_DATA(header).cast::<libc::c_int>()) };
    let mut received = unsafe { std::fs::File::from_raw_fd(fd) };
    if !mixed {
        let mut contents = String::new();
        received.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "test resource");
    }
    if two_resources {
        assert_eq!(unsafe { (*header).cmsg_len }, unsafe {
            libc::CMSG_LEN(
                ((if mixed { 3 } else { 2 }) * std::mem::size_of::<libc::c_int>()) as u32,
            ) as usize
        });
        let second = unsafe { *(libc::CMSG_DATA(header).cast::<libc::c_int>().add(1)) };
        let mut received = unsafe { std::fs::File::from_raw_fd(second) };
        if !mixed {
            let mut contents = String::new();
            received.read_to_string(&mut contents).unwrap();
            assert_eq!(contents, "second resource");
        } else {
            let third = unsafe { *(libc::CMSG_DATA(header).cast::<libc::c_int>().add(2)) };
            let third = unsafe { std::os::fd::OwnedFd::from_raw_fd(third) };
            assert!(carosign_linux::sys::is_sealed(&third).unwrap());
            let mut config = std::fs::File::from(third);
            let mut contents = String::new();
            config.read_to_string(&mut contents).unwrap();
            assert_eq!(contents, "test resource");
        }
        std::fs::remove_file(second_path).unwrap();
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
    let _ = recv(&tunnel);
    let ready = frame(
        3,
        Payload::AuthenticatorReady(provider::AuthenticatorReady {
            authenticator: Some(auth.clone()),
            capabilities: None,
        }),
    );
    worker.send(&ready.encode_to_vec()).unwrap();
    let _ = recv(&tunnel);
    if let Some(pair) = control {
        drop(pair.worker);
        let control = PacketSocket::new(pair.platform).unwrap();
        let request = relay::Control {
            request_id: vec![11; 16],
            action: Some(relay::control::Action::Revoke(relay::Revoke {
                scope: Some(scope),
            })),
        };
        control.send(&request.encode_to_vec()).unwrap();
        let reply = relay::ControlResult::decode(recv(&control).as_slice()).unwrap();
        assert_eq!(reply.request_id, vec![11; 16]);
        assert_eq!(
            reply.result,
            Some(relay::control_result::Result::ExclusiveQuiesced(true))
        );
        drop(control);
        assert!(!child.wait().unwrap().success());
        owner_thread.unwrap().join().unwrap();
        return;
    }
    let subject = common::SubjectRef {
        authority_id: vec![10; 16],
        subject_id: vec![11; 16],
    };
    if continuation {
        let mut prepare = frame(0, Payload::PrepareEnrollmentChallenge(provider::PrepareEnrollmentChallenge {
            authenticator: Some(auth.clone()), subject: Some(subject.clone()), mutation_id: vec![13; 16],
        }));
        prepare.command_id = vec![14; 16];
        tunnel.send(&envelope(4, Body::Command(relay::Command { request_id: request(4), frame: Some(prepare), mutation_permit: None }))).unwrap();
        let received = provider::ProviderFrame::decode(recv(&worker).as_slice()).unwrap();
        assert!(matches!(received.payload, Some(Payload::PrepareEnrollmentChallenge(_))));
        assert_eq!(received.uid, Some(1000));
        assert!(matches!(relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body, Some(Body::CommandResult(_))));
        let mut ack = frame(4, Payload::CommandAck(provider::CommandAck { accepted: true, rejection: None }));
        ack.command_id = vec![14; 16];
        ack.uid = None;
        worker.send(&ack.encode_to_vec()).unwrap();
        assert!(matches!(relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body, Some(Body::Event(_))));
    }
    let planned = common::EnrollmentRef {
        enrollment_id: vec![12; 16],
        authenticator_id: vec![5; 16],
        generation: 1,
    };
    let enrollment = provider::StartEnrollment {
        authenticator: Some(auth),
        subject: Some(subject),
        mutation_id: vec![13; 16],
        planned_enrollment: Some(planned),
        label: "test".into(),
        enrollment_authorization_ref: if continuation { vec![15; 16] } else { vec![] },
        enrollment_challenge_digest: if continuation { vec![16; 32] } else { vec![] },
        ..Default::default()
    };
    let mutation = frame(
        0,
        if continuation {
            Payload::ContinueEnrollment(provider::ContinueEnrollment {
                enrollment: Some(enrollment),
            })
        } else {
            Payload::StartEnrollment(enrollment)
        },
    );
    let mut scope_key = vec![10; 16];
    scope_key.extend([11; 16]);
    scope_key.extend([5; 16]);
    let permit = relay::MutationPermit {
        lease_id: vec![6; 16],
        scope: scope_key,
        mutation_id: vec![13; 16],
    };
    let command = |seq| {
        envelope(
            seq,
            Body::Command(relay::Command {
                request_id: request(seq),
                frame: Some(mutation.clone()),
                mutation_permit: Some(permit.clone()),
            }),
        )
    };
    let seq = if continuation { 5 } else { 4 };
    tunnel.send(&command(seq)).unwrap();
    let delivered = provider::ProviderFrame::decode(recv(&worker).as_slice()).unwrap();
    assert!(matches!(
        (continuation, delivered.payload),
        (false, Some(Payload::StartEnrollment(_))) | (true, Some(Payload::ContinueEnrollment(_)))
    ));
    let receipt = relay::Frame::decode(recv(&tunnel).as_slice()).unwrap();
    assert!(matches!(
        receipt.body,
        Some(Body::CommandResult(relay::CommandResult {
            result: Some(relay::command_result::Result::Sent(true)),
            ..
        }))
    ));
    tunnel.send(&command(seq + 1)).unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(worker.receive().is_err());
    if let Some(thread) = owner_thread {
        thread.join().unwrap();
    }
}

/// Mock is a separate program; the resolution in the test sends a trusted fixture.
/// Receiving the present ArmedMutationTicket It is checked separately in the vault.
#[test]
#[ignore = "нужен собранный mock в CAROSIGN_MOCK_PROVIDER_TEST_BINARY"]
fn process_real_mock_enrollment_and_removal() {
    run_real_mock(false);
    run_real_mock(true);
}

fn run_real_mock(removal: bool) {
    use provider::provider_frame::Payload;
    use relay::frame::Body;
    let binary = std::env::var_os("CAROSIGN_MOCK_PROVIDER_TEST_BINARY").unwrap();
    let tunnel = socketpair_seqpacket().unwrap();
    let provider = socketpair_seqpacket().unwrap();
    for fd in [tunnel.worker.as_raw_fd(), provider.platform.as_raw_fd()] {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    let mut broker = std::process::Command::new(env!("CARGO_BIN_EXE_carosign-broker"))
        .args([
            "--tunnel-fd",
            &tunnel.worker.as_raw_fd().to_string(),
            "--provider-fd",
            &provider.platform.as_raw_fd().to_string(),
            "--worker",
            "01010101010101010101010101010101",
            "--epoch",
            "7",
            "--broker",
            "03030303030303030303030303030303",
            "--boot",
            "04040404040404040404040404040404",
        ])
        .spawn()
        .unwrap();
    drop(tunnel.worker);
    drop(provider.platform);
    let fd = provider.worker.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert_eq!(
        unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
        0
    );
    let mut mock = std::process::Command::new(binary)
        .args([
            fd.to_string(),
            if removal {
                "seeded-no-sample".into()
            } else {
                "no-sample".into()
            },
        ])
        .spawn()
        .unwrap();
    drop(provider.worker);
    let tunnel = PacketSocket::new(tunnel.platform).unwrap();
    tunnel
        .send(&envelope(
            1,
            Body::Hello(relay::Hello {
                broker_instance: vec![3; 16],
                boot_id: vec![4; 16],
            }),
        ))
        .unwrap();
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::Hello(_))
    ));
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::Event(_))
    ));
    let mut init = frame(
        0,
        Payload::Initialize(provider::Initialize {
            worker_instance_id: vec![1; 16],
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: vec![2; 16],
            profile_id: vec![3; 16],
            enabled_features: vec![provider::Feature::BasicOperations as i32],
        }),
    );
    init.operation_id.clear();
    init.command_id.clear();
    init.device_epoch = 0;
    tunnel
        .send(&envelope(
            2,
            Body::Command(relay::Command {
                request_id: request(2),
                frame: Some(init),
                mutation_permit: None,
            }),
        ))
        .unwrap();
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::CommandResult(_))
    ));
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::Event(_))
    ));
    let auth = common::AuthenticatorRef {
        authenticator_id: vec![5; 16],
        device_epoch: 7,
    };
    let scope = relay::Scope {
        lease_id: vec![6; 16],
        owner: Some(relay::Owner {
            operation_id: vec![8; 16],
            worker_instance: vec![1; 16],
            authenticator_id: vec![5; 16],
            device_epoch: 7,
        }),
        claims: vec![relay::Claim {
            resource_id: vec![7; 16],
            mode: relay::AccessMode::Exclusive as i32,
        }],
    };
    let mut bind = frame(
        0,
        Payload::BindAuthenticator(provider::BindAuthenticator {
            authenticator: Some(auth.clone()),
            profile_id: vec![3; 16],
            lease_id: vec![6; 16],
            fd_slots: vec![],
        }),
    );
    bind.command_id = vec![4; 16];
    tunnel
        .send(&envelope(
            3,
            Body::Bind(relay::Bind {
                request_id: request(3),
                scope: Some(scope),
                slots: vec![],
                frame: Some(bind),
            }),
        ))
        .unwrap();
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::BindResult(_))
    ));
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::Event(_))
    ));
    let subject = common::SubjectRef {
        authority_id: vec![11; 16],
        subject_id: vec![12; 16],
    };
    let enrollment = common::EnrollmentRef {
        enrollment_id: vec![15; 16],
        authenticator_id: vec![5; 16],
        generation: 1,
    };
    let payload = if removal {
        Payload::StartRemoval(provider::StartRemoval {
            authenticator: Some(auth),
            subject: Some(subject),
            mutation_id: vec![13; 16],
            enrollment: Some(enrollment.clone()),
            provider_opaque_ref: vec![21; 16],
            enrollment_binding: Some(provider::EnrollmentBinding {
                enrollment: Some(enrollment),
                kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: vec![21; 16],
                ..Default::default()
            }),
        })
    } else {
        Payload::StartEnrollment(provider::StartEnrollment {
            authenticator: Some(auth),
            subject: Some(subject),
            mutation_id: vec![13; 16],
            planned_enrollment: Some(enrollment),
            label: "test".into(),
            ..Default::default()
        })
    };
    let mut scope_key = vec![11; 16];
    scope_key.extend([12; 16]);
    scope_key.extend([5; 16]);
    let permit = relay::MutationPermit {
        lease_id: vec![6; 16],
        scope: scope_key,
        mutation_id: vec![13; 16],
    };
    tunnel
        .send(&envelope(
            4,
            Body::Command(relay::Command {
                request_id: request(4),
                frame: Some(frame(0, payload)),
                mutation_permit: Some(permit),
            }),
        ))
        .unwrap();
    assert!(matches!(
        relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body,
        Some(Body::CommandResult(_))
    ));
    let mut completed = false;
    for _ in 0..2 {
        if let Some(Body::Event(relay::Event {
            frame:
                Some(provider::ProviderFrame {
                    payload: Some(Payload::MutationOutcome(outcome)),
                    ..
                }),
        })) = relay::Frame::decode(recv(&tunnel).as_slice()).unwrap().body
        {
            assert_eq!(outcome.mutation_id, vec![13; 16]);
            assert_eq!(
                outcome.status,
                provider::MutationStatus::MutationProviderCompleted as i32
            );
            completed = true;
        }
    }
    assert!(completed);
    drop(tunnel);
    assert!(!broker.wait().unwrap().success());
    assert!(mock.wait().unwrap().success());
}
