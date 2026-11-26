use super::super::gate::PolicyAdmissionError;
use crate::policy::PolicyChangeWatch;
use std::io;
use std::os::unix::net::UnixStream;

use super::*;
use crate::process::gate::Gate;
use std::os::fd::AsRawFd;
use std::time::Duration;

#[test]
fn installer_update_client_process() {
    use std::io::{Read, Write};
    let Some(path) = std::env::var_os("CAROSIGN_INSTALLER_TEST_SOCKET") else {
        return;
    };
    let mut stream = UnixStream::connect(path).unwrap();
    stream.write_all(&6_u32.to_be_bytes()).unwrap();
    stream.write_all(b"second").unwrap();
    let mut reply = [0; 9];
    stream.read_exact(&mut reply).unwrap();
    assert_eq!(reply[0], 0);
    assert_eq!(u64::from_be_bytes(reply[1..].try_into().unwrap()), 2);
}

#[test]
fn installer_process_requests_update_from_owner() {
    use std::os::unix::net::UnixListener;
    let dir =
        std::env::temp_dir().join(format!("carosign-installer-owner-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let socket = dir.join("control");
    let listener = UnixListener::bind(&socket).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, b"first").unwrap();
    let owner = PolicyOwner::new(1, PolicyChangeWatch::new(&path).unwrap()).unwrap();
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let id = owner.register(1, sender).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process::policy_admissions::tests::installer_update_client_process",
        ])
        .env("CAROSIGN_INSTALLER_TEST_SOCKET", &socket)
        .spawn()
        .unwrap();
    let (stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    serve_installer_update(
        &owner,
        stream,
        |_| Ok(true),
        |payload| {
            std::fs::write(&path, payload)?;
            PolicyChangeWatch::new(&path)
        },
    )
    .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    assert_eq!(
        owner.admit(1, id),
        Err(PolicyAdmissionError::StaleGeneration)
    );
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let id = owner.register(2, sender).unwrap();
    owner.admit(2, id).unwrap();
    assert!(receiver.wait(Duration::from_secs(1)).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn untrusted_installer_cannot_update_policy() {
    let dir =
        std::env::temp_dir().join(format!("carosign-installer-denied-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, b"first").unwrap();
    let owner = PolicyOwner::new(1, PolicyChangeWatch::new(&path).unwrap()).unwrap();
    let (server, _client) = UnixStream::pair().unwrap();
    let result = serve_installer_update(&owner, server, |_| Ok(false), |_| panic!("denied update"));
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(std::fs::read(&path).unwrap(), b"first");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn policy_client_process() {
    use std::io::{Read, Write};
    let Some(path) = std::env::var_os("CAROSIGN_POLICY_TEST_SOCKET") else {
        return;
    };
    let mut stream = std::os::unix::net::UnixStream::connect(path).unwrap();
    stream.write_all(b"R").unwrap();
    let mut response = [0];
    stream.read_exact(&mut response).unwrap();
    assert_eq!(response, *b"2");
    stream.write_all(b"A").unwrap();
    stream.read_exact(&mut response).unwrap();
    assert_eq!(response, *b"S");
}

#[test]
fn owner_orders_cross_process_update_and_admission() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    let dir = std::env::temp_dir().join(format!("carosign-owner-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let socket = dir.join("control");
    let listener = UnixListener::bind(&socket).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, b"first").unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process::policy_admissions::tests::policy_client_process",
        ])
        .env("CAROSIGN_POLICY_TEST_SOCKET", &socket)
        .spawn()
        .unwrap();
    let (mut control, _) = listener.accept().unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let owner = PolicyOwner::new(1, PolicyChangeWatch::new(&path).unwrap()).unwrap();
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let id = owner.register(1, sender).unwrap();
    let mut request = [0];
    control.read_exact(&mut request).unwrap();
    assert_eq!(request, *b"R");
    assert_eq!(
        owner.replace(|| {
            std::fs::write(&path, b"second")?;
            PolicyChangeWatch::new(&path)
        }),
        Ok(2)
    );
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    control.write_all(b"2").unwrap();
    control.read_exact(&mut request).unwrap();
    assert_eq!(request, *b"A");
    assert_eq!(
        owner.admit(1, id),
        Err(PolicyAdmissionError::StaleGeneration)
    );
    control.write_all(b"S").unwrap();
    assert!(child.wait().unwrap().success());
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let id = owner.register(2, sender).unwrap();
    assert_eq!(owner.admit(2, id), Ok(()));
    assert!(receiver.wait(Duration::from_secs(1)).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_update_never_reopens_owner() {
    let dir = std::env::temp_dir().join(format!("carosign-owner-fail-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, b"first").unwrap();
    let owner = PolicyOwner::new(1, PolicyChangeWatch::new(&path).unwrap()).unwrap();
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let id = owner.register(1, sender).unwrap();
    assert_eq!(
        owner.replace(|| Err(std::io::ErrorKind::PermissionDenied.into())),
        Err(PolicyAdmissionError::Io)
    );
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    assert_eq!(owner.admit(1, id), Err(PolicyAdmissionError::Closed));
    assert_eq!(
        owner.replace(|| PolicyChangeWatch::new(&path)),
        Err(PolicyAdmissionError::Closed)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn changed_file_closes_all_gates_and_blocks_new_admissions() {
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open("/dev/shm/carosign-policy-tests.lock")
        .unwrap();
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    let dir = std::path::Path::new("/dev/shm")
        .join(format!("carosign-watched-gates-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, b"first").unwrap();
    let watch = PolicyChangeWatch::new(&path).unwrap();
    let owner = WatchedPolicyAdmissions::new(1, watch).unwrap();
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let id = owner.register(1, sender).unwrap();
    std::fs::write(&path, b"changed").unwrap();
    assert_eq!(owner.admit(1, id), Err(PolicyAdmissionError::Closed));
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    assert_eq!(owner.register(1, sender), Err(PolicyAdmissionError::Closed));
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
