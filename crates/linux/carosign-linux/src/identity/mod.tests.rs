use super::*;
use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};

#[test]
#[ignore = "вспомогательный отдельный процесс peer"]
fn peer_process_child() {
    let path = std::env::var_os("CAROSIGN_TEST_PEER_SOCKET").expect("путь сокета");
    let mut stream = UnixStream::connect(path).expect("подключение");
    let mut byte = [0u8; 1];
    let _ = stream.read(&mut byte);
}

#[test]
#[ignore = "требует системную шину и systemd GetUnitByPIDFD"]
fn systemd_observes_live_socket_peer() {
    let (socket, other) = UnixStream::pair().unwrap();
    let peer = ObservedPeer::capture(socket.into()).unwrap();
    let bus = dbus::blocking::Connection::new_system().unwrap();
    let (unit, invocation) = peer.observe_systemd_unit(&bus).unwrap();
    assert!(!unit.is_empty());
    assert_ne!(invocation, [0; 16]);
    peer.matches_systemd_invocation(&bus, &unit, invocation)
        .unwrap();
    assert!(
        peer.matches_systemd_invocation(&bus, &unit, [0; 16])
            .is_err()
    );
    drop(other);
}

#[test]
fn observed_invocation_rejects_restarts_and_wrong_unit() {
    let first = SystemdObservation {
        owner: String::from(":1.7"),
        unit: String::from("carosignd.service"),
        invocation: [1; 16],
    };
    assert!(matching_invocation(
        &first,
        &first,
        "carosignd.service",
        [1; 16]
    ));
    let restarted = SystemdObservation {
        owner: first.owner.clone(),
        unit: first.unit.clone(),
        invocation: [2; 16],
    };
    assert!(!matching_invocation(
        &first,
        &restarted,
        "carosignd.service",
        [1; 16]
    ));
    assert!(!matching_invocation(
        &first,
        &first,
        "other.service",
        [1; 16]
    ));
    // Between observations, the manager changed, even if the unit and invocation were the
    // same: the source of the second answer is no longer trusted.
    let replaced_manager = SystemdObservation {
        owner: String::from(":1.8"),
        unit: first.unit.clone(),
        invocation: first.invocation,
    };
    assert!(!matching_invocation(
        &first,
        &replaced_manager,
        "carosignd.service",
        [1; 16]
    ));
    assert!(!matching_invocation(
        &first,
        &first,
        "carosignd.service",
        [0; 16]
    ));
}

#[test]
fn peer_pidfd_tracks_connected_child_until_exit() {
    let dir = std::env::temp_dir().join(format!("carosign-peer-{}", std::process::id()));
    std::fs::create_dir(&dir).expect("создание каталога");
    let path = dir.join("peer.sock");
    let listener = UnixListener::bind(&path).expect("слушатель");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "identity::tests::peer_process_child",
            "--ignored",
        ])
        .env("CAROSIGN_TEST_PEER_SOCKET", &path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (stream, _) = listener.accept().expect("соединение");
    let observed = ObservedPeer::capture(stream.into());
    match observed {
        Ok(peer) => {
            assert_eq!(peer.credentials().pid, child.id());
            assert!(!peer.has_exited().unwrap());
            // The executable fixture file is not approved by a signed source and should not
            // be taken by random measurement.
            assert!(
                peer.matches_executable_verity(peer.credentials().uid, &[0; 32])
                    .is_err()
            );
            assert!(
                peer.matches_executable_verity(peer.credentials().uid.wrapping_add(1), &[1; 32])
                    .is_err()
            );
            assert!(
                peer.matches_executable_verity(peer.credentials().uid, &[1; 32])
                    .is_err()
            );
            child.kill().unwrap();
            child.wait().unwrap();
            assert!(peer.has_exited().unwrap());
            assert!(
                peer.matches_executable_verity(peer.credentials().uid, &[1; 32])
                    .is_err()
            );
        }
        Err(error)
            if matches!(
                error.errno,
                libc::ENOPROTOOPT | libc::EOPNOTSUPP | libc::EINVAL
            ) =>
        {
            // The old kernel does not give a stable link; there is no PID bypass.
            child.kill().unwrap();
            child.wait().unwrap();
        }
        Err(error) => panic!("неожиданная ошибка: {error}"),
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
