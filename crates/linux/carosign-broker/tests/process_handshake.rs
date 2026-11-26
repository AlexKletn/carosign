//! Verification of this executable broker on the transferred connections.

use carosign_broker::transport::PacketSocket;
use carosign_linux::sys::socketpair_seqpacket;
use carosign_linux_ipc::provider::tunnel::TunnelClient;
use carosign_protocol::carosign::provider::v1::{
    Feature, Initialize, ProviderFrame, WorkerHello, provider_frame::Payload,
};
use prost::Message;
use std::os::fd::AsRawFd;
use std::time::Duration;

#[test]
fn supervisor_sockets_exchange_hello_and_worker_event() {
    let tunnel = socketpair_seqpacket().unwrap();
    let provider = socketpair_seqpacket().unwrap();
    for fd in [tunnel.worker.as_raw_fd(), provider.platform.as_raw_fd()] {
        // SAFETY: The test descriptors are changed.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
            0
        );
    }
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_carosign-broker"))
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
    let mut client = TunnelClient::connect(
        tunnel.platform,
        [1; 16],
        7,
        [3; 16],
        [4; 16],
        Duration::from_secs(2),
    )
    .unwrap();
    let worker = PacketSocket::new(provider.worker).unwrap();
    worker
        .send(
            &ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                frame_seq: 1,
                payload: Some(Payload::WorkerHello(WorkerHello {
                    major: 1,
                    min_minor: carosign_protocol::limits::MINOR_BASELINE,
                    max_minor: carosign_protocol::limits::MINOR_MAX,
                    ..Default::default()
                })),
                ..Default::default()
            }
            .encode_to_vec(),
        )
        .unwrap();
    assert!(matches!(
        client
            .receive_event(Duration::from_secs(2))
            .unwrap()
            .payload,
        Some(Payload::WorkerHello(_))
    ));
    let (release, wait) = std::sync::mpsc::channel();
    let worker_thread = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match worker.receive() {
                Ok(packet) => {
                    let frame = ProviderFrame::decode(packet.as_slice()).unwrap();
                    assert_eq!(frame.frame_seq, 1);
                    assert!(matches!(frame.payload, Some(Payload::Initialize(_))));
                    break;
                }
                Err(carosign_broker::transport::TransportError::WouldBlock)
                    if std::time::Instant::now() < deadline =>
                {
                    std::thread::yield_now()
                }
                other => panic!("ожидался Initialize: {other:?}"),
            }
        }
        wait.recv().unwrap();
    });
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
            Duration::from_secs(2),
        )
        .unwrap();
    drop(client);
    release.send(()).unwrap();
    worker_thread.join().unwrap();
    assert!(!child.wait().unwrap().success());
}
