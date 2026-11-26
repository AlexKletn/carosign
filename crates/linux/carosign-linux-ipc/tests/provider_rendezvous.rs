use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::ids::{AuthenticatorId, WorkerInstanceId};
use carosign_linux::sys;
use carosign_linux_ipc::provider_rendezvous::{WorkerRequest, receive_owner_channel};
use carosign_protocol::carosign::{provider::v1 as provider, provider_rendezvous::v1 as wire};
use prost::Message;

const BASE: &str = "/run/carosign-rendezvous";
const CONTROL: &str = "/run/carosign-rendezvous/provider/control.sock";
const FIXTURE: &str = "/run/carosign-rendezvous/fixture";
const TIMEOUT: Duration = Duration::from_secs(3);

fn spawn_fixture(test: &str) -> Command {
    let mut command = Command::new(FIXTURE);
    command.args(["--ignored", "--exact", test, "--nocapture"]);
    command
}

fn send(fd: &std::os::fd::OwnedFd, bytes: &[u8]) {
    // SAFETY: fd and buffer are valid at the time of the system call.
    let sent = unsafe {
        libc::send(
            fd.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    assert_eq!(sent, bytes.len() as isize);
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !predicate() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn finish(child: &mut Child) {
    wait_until(|| child.try_wait().unwrap().is_some());
    assert!(child.wait().unwrap().success());
}

#[test]
#[ignore = "запускается в отдельном user и mount namespace с приватным /run"]
fn open_session_preserves_worker_hello_and_rejects_bad_proof() {
    std::fs::create_dir_all(format!("{BASE}/provider")).unwrap();
    std::fs::copy(std::env::current_exe().unwrap(), FIXTURE).unwrap();
    std::fs::set_permissions(FIXTURE, std::fs::Permissions::from_mode(0o755)).unwrap();
    let provider_directory = format!("{BASE}/provider");
    std::os::unix::fs::chown(&provider_directory, Some(65534), Some(65534)).unwrap();
    let mut daemon = spawn_fixture("provider_daemon")
        .uid(65534)
        .gid(65534)
        .spawn()
        .unwrap();
    wait_until(|| Path::new(CONTROL).exists());

    let request = WorkerRequest {
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([1; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
        worker_id: WorkerInstanceId::from_bytes([2; 16]).unwrap(),
        uid: 65534,
        gid: 65534,
        declaration_only: true,
    };
    let control = sys::connect_seqpacket(Path::new(CONTROL)).unwrap();
    let mut worker = request.open(&control, Path::new(BASE), TIMEOUT).unwrap();
    assert!(!worker.has_exited().unwrap());
    let fd = worker.take_socket().unwrap();
    assert!(worker.take_socket().is_none());
    let (mut broker, mut tunnel) = attach_broker(fd, &request);
    let frame = tunnel.receive_event(TIMEOUT).unwrap();
    assert!(matches!(
        frame.payload,
        Some(provider::provider_frame::Payload::WorkerHello(_))
    ));
    worker.stop().unwrap();
    wait_until(|| worker.has_exited().unwrap());
    tunnel.abort();
    wait_until(|| broker.try_wait().unwrap().is_some());
    assert_eq!(broker.wait().unwrap().code(), Some(1));

    let request = WorkerRequest {
        declaration_only: false,
        ..request
    };
    let owner = sys::socketpair_seqpacket().unwrap();
    let control = sys::connect_seqpacket(Path::new(CONTROL)).unwrap();
    let mut worker = request
        .open_with_owner_channel(&control, Path::new(BASE), TIMEOUT, &owner.worker)
        .unwrap();
    drop(owner.worker);
    let mut data = [0; 16];
    let size = sys::receive_seqpacket(&owner.platform, &mut data, TIMEOUT).unwrap();
    assert_eq!(&data[..size], b"owner");
    let (mut broker, mut tunnel) = attach_broker(worker.take_socket().unwrap(), &request);
    assert!(matches!(
        tunnel.receive_event(TIMEOUT).unwrap().payload,
        Some(provider::provider_frame::Payload::WorkerHello(_))
    ));
    worker.stop().unwrap();
    wait_until(|| worker.has_exited().unwrap());
    tunnel.abort();
    wait_until(|| broker.try_wait().unwrap().is_some());

    let owner = sys::socketpair_seqpacket().unwrap();
    let control = sys::connect_seqpacket(Path::new(CONTROL)).unwrap();
    let refused =
        request.open_with_owner_channel(&control, Path::new(BASE), TIMEOUT, &owner.worker);
    assert!(matches!(refused, Err(error) if error.kind() == io::ErrorKind::InvalidData));
    drop(owner.worker);
    assert!(sys::receive_seqpacket(&owner.platform, &mut data, TIMEOUT).is_err());

    let control = sys::connect_seqpacket(Path::new(CONTROL)).unwrap();
    let failed = request.open(&control, Path::new(BASE), TIMEOUT);
    assert!(matches!(failed, Err(error) if error.kind() == io::ErrorKind::InvalidData));
    assert!(std::fs::read_dir(BASE).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("worker-")
    }));
    finish(&mut daemon);
}

fn attach_broker(
    fd: std::os::fd::OwnedFd,
    request: &WorkerRequest,
) -> (Child, carosign_linux_ipc::provider::tunnel::TunnelClient) {
    use carosign_linux_ipc::provider::tunnel::TunnelClient;
    let pair = sys::socketpair_seqpacket().unwrap();
    let fds = [pair.worker.as_raw_fd(), fd.as_raw_fd()];
    let worker = request
        .worker_id
        .as_bytes()
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect::<String>();
    let mut command =
        Command::new(std::env::var_os("CAROSIGN_TEST_BROKER").expect("путь к брокеру"));
    command.args([
        "--tunnel-fd",
        &fds[0].to_string(),
        "--provider-fd",
        &fds[1].to_string(),
        "--worker",
        &worker,
        "--epoch",
        "1",
        "--broker",
        "03030303030303030303030303030303",
        "--boot",
        "04040404040404040404040404040404",
        "--discovered-worker",
    ]);
    // SAFETY: In the child process, only the two FD flags are changed.
    unsafe {
        command.pre_exec(move || {
            for fd in fds {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let broker = command.spawn().unwrap();
    drop(pair.worker);
    drop(fd);
    let tunnel = TunnelClient::connect(
        pair.platform,
        *request.worker_id.as_bytes(),
        1,
        [3; 16],
        [4; 16],
        TIMEOUT,
    )
    .unwrap();
    (broker, tunnel)
}

#[test]
#[ignore = "дочерняя точка входа"]
fn provider_daemon() {
    let listener = sys::listen_unix(CONTROL).unwrap();
    for (expected_channel, mismatch, wrong_ack) in [
        (false, false, false),
        (true, false, false),
        (true, false, true),
        (false, true, false),
    ] {
        let (control, _) = sys::accept_seqpacket_until(&listener, TIMEOUT).unwrap();
        let mut buffer = [0; 1024];
        let size = sys::receive_seqpacket(&control, &mut buffer, TIMEOUT).unwrap();
        let request = wire::Request::decode(&buffer[..size]).unwrap();
        let Some(wire::request::Action::OpenSession(session)) = request.action else {
            panic!("expected session");
        };
        assert_eq!(session.owner_channel_required, expected_channel);
        let mut proof = wire::WorkerProof {
            version: 1,
            request_id: request.request_id.clone(),
            authenticator_id: session.authenticator_id,
            device_epoch: session.device_epoch,
            worker_instance_id: session.worker_instance_id,
            nonce: session.nonce,
        };
        if mismatch {
            proof.nonce[0] ^= 1;
        }
        let mut child = spawn_fixture("provider_worker")
            .env("CAROSIGN_WORKER_SOCKET", session.worker_socket)
            .env(
                "CAROSIGN_WORKER_PROOF",
                proof
                    .encode_to_vec()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            )
            .env("CAROSIGN_WORKER_MISMATCH", if mismatch { "yes" } else { "no" })
            .env(
                "CAROSIGN_WORKER_CHANNEL",
                if expected_channel { "yes" } else { "no" },
            )
            .env("CAROSIGN_WORKER_WRONG_ACK", if wrong_ack { "yes" } else { "no" })
            .spawn()
            .unwrap();
        send(
            &control,
            &wire::Response {
                version: 1,
                request_id: request.request_id,
                result: Some(wire::response::Result::SessionOpening(
                    wire::SessionOpening {},
                )),
            }
            .encode_to_vec(),
        );
        let status = child.wait().unwrap();
        assert_eq!(status.success(), mismatch);
    }
}

#[test]
#[ignore = "дочерняя точка входа"]
fn provider_worker() {
    let path = std::env::var("CAROSIGN_WORKER_SOCKET").unwrap();
    let proof = std::env::var("CAROSIGN_WORKER_PROOF").unwrap();
    let proof = (0..proof.len())
        .step_by(2)
        .map(|n| u8::from_str_radix(&proof[n..n + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let fd = sys::connect_seqpacket(Path::new(&path)).unwrap();
    send(&fd, &proof);
    if std::env::var("CAROSIGN_WORKER_MISMATCH").unwrap() == "yes" {
        return;
    }
    if std::env::var("CAROSIGN_WORKER_CHANNEL").unwrap() == "yes" {
        if std::env::var("CAROSIGN_WORKER_WRONG_ACK").unwrap() == "yes" {
            let _channel = sys::receive_fd_until(&fd, TIMEOUT).unwrap();
            send(&fd, &[2]);
            std::thread::sleep(TIMEOUT);
            return;
        }
        let channel = receive_owner_channel(&fd, TIMEOUT).unwrap();
        send(&channel, b"owner");
    }
    send(
        &fd,
        &provider::ProviderFrame {
            major: 1,
            minor: 4,
            frame_seq: 1,
            payload: Some(provider::provider_frame::Payload::WorkerHello(
                provider::WorkerHello {
                    major: 1,
                    min_minor: 4,
                    max_minor: 4,
                    ..Default::default()
                },
            )),
            ..Default::default()
        }
        .encode_to_vec(),
    );
    std::thread::sleep(TIMEOUT);
}
