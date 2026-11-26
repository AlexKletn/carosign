use crate::sys;
use crate::sys::FileIdentity;
use carosign_core::ids::PackageId;
use carosign_core::ids::ProfileId;
use carosign_core::ids::ProviderId;
use std::path::PathBuf;

use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

struct Fixture {
    root: PathBuf,
    owner: u32,
    key: InstallationKey,
    record_path: PathBuf,
    root_identity: FileIdentity,
    identity: FileIdentity,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "carosign-installed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(root.join("bin")).unwrap();
        let executable = root.join("bin/worker");
        std::fs::write(&executable, b"fixture").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let fd = sys::open_launch_candidate(executable.to_str().unwrap()).unwrap();
        let identity = sys::launch_metadata(&fd).unwrap().identity;
        let root_fd = sys::open_launch_candidate(root.to_str().unwrap()).unwrap();
        let root_identity = sys::launch_metadata(&root_fd).unwrap().identity;
        Self {
            root,
            owner: identity.uid,
            key: InstallationKey {
                package: PackageId::from_bytes([1; 16]).unwrap(),
                provider: ProviderId::from_bytes([2; 16]).unwrap(),
                profile: ProfileId::from_bytes([3; 16]).unwrap(),
            },
            record_path: PathBuf::from("bin/worker"),
            root_identity,
            identity,
        }
    }

    fn pin(&self, requested: InstallationKey) -> Result<PinnedCandidate, InstallationError> {
        let root = sys::open_launch_candidate(self.root.to_str().unwrap()).unwrap();
        pin_candidate(self, requested, root, self.owner)
    }
}

impl InstallationAuthority for Fixture {
    fn approved(&self, _: InstallationKey) -> Result<ApprovedInstallation, InstallationError> {
        Ok(ApprovedInstallation {
            key: self.key,
            executable: self.record_path.clone(),
            root_identity: self.root_identity,
            executable_identity: self.identity,
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn absent_system_authority_cannot_issue_an_installation() {
    let fixture = Fixture::new();
    assert!(matches!(
        UnavailableInstallationAuthority.approved(fixture.key),
        Err(InstallationError::Unavailable)
    ));
}

#[test]
fn pins_exact_object_and_rejects_wrong_record_or_replacement() {
    let fixture = Fixture::new();
    let pinned = fixture.pin(fixture.key).ok().unwrap();
    assert_eq!(pinned.identity, fixture.identity);
    let mut wrong = fixture.key;
    wrong.profile = ProfileId::from_bytes([8; 16]).unwrap();
    assert!(matches!(
        fixture.pin(wrong),
        Err(InstallationError::MismatchedRecord)
    ));
    let other_root = Fixture::new();
    let other_fd = sys::open_launch_candidate(other_root.root.to_str().unwrap()).unwrap();
    assert!(matches!(
        pin_candidate(&fixture, fixture.key, other_fd, fixture.owner),
        Err(InstallationError::Replaced)
    ));

    std::fs::rename(
        fixture.root.join("bin/worker"),
        fixture.root.join("bin/old"),
    )
    .unwrap();
    std::fs::write(fixture.root.join("bin/worker"), b"replacement").unwrap();
    std::fs::set_permissions(
        fixture.root.join("bin/worker"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert!(matches!(
        fixture.pin(fixture.key),
        Err(InstallationError::Replaced)
    ));
    assert_eq!(
        sys::launch_metadata(&pinned._executable).unwrap().identity,
        fixture.identity
    );
    assert_eq!(
        sys::launch_metadata(&pinned.readable_executable)
            .unwrap()
            .identity,
        fixture.identity
    );
    let mut bytes = [0u8; 7];
    use std::os::unix::fs::FileExt;
    let readable: std::fs::File = pinned.readable_executable.into();
    assert_eq!(readable.read_at(&mut bytes, 0).unwrap(), 7);
    assert_eq!(&bytes, b"fixture");
}

#[test]
fn rejects_parent_link_and_writable_component() {
    let mut fixture = Fixture::new();
    fixture.record_path = PathBuf::from("../worker");
    assert!(matches!(
        fixture.pin(fixture.key),
        Err(InstallationError::InvalidPath)
    ));
    fixture.record_path = PathBuf::from("link/worker");
    symlink("bin", fixture.root.join("link")).unwrap();
    assert!(matches!(
        fixture.pin(fixture.key),
        Err(InstallationError::Kernel(libc::ELOOP))
    ));
    fixture.record_path = PathBuf::from("bin/worker");
    symlink("worker", fixture.root.join("bin/worker-link")).unwrap();
    fixture.record_path = PathBuf::from("bin/worker-link");
    assert!(matches!(
        fixture.pin(fixture.key),
        Err(InstallationError::Kernel(libc::ELOOP))
    ));
    fixture.record_path = PathBuf::from("bin/worker");
    std::fs::set_permissions(
        fixture.root.join("bin"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(matches!(
        fixture.pin(fixture.key),
        Err(InstallationError::UnsafeObject)
    ));
}

#[test]
fn nonregular_pinned_object_is_not_reopened() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new();
    let fifo = fixture.root.join("fifo");
    let path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: The path is valid; the FIFO is in an isolated test catalog.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let pinned = sys::open_launch_candidate(fifo.to_str().unwrap()).unwrap();
    assert_eq!(
        sys::reopen_pinned_regular(&pinned).unwrap_err().errno,
        libc::EINVAL
    );
}

#[test]
fn candidate_readable_fd_executes_pinned_object_after_path_replacement() {
    use std::os::fd::AsFd;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    const CHILD: &str = "CAROSIGN_INSTALLED_EXEC_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mut fixture = Fixture::new();
        let path = fixture.root.join("bin/worker");
        std::fs::copy("/bin/true", &path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let fd = sys::open_launch_candidate(path.to_str().unwrap()).unwrap();
        fixture.identity = sys::launch_metadata(&fd).unwrap().identity;
        let pinned = fixture.pin(fixture.key).ok().unwrap();
        std::fs::rename(&path, fixture.root.join("bin/old")).unwrap();
        std::fs::write(&path, b"#!/bin/sh\nexit 42\n").unwrap();
        drop(fixture);
        crate::process::pinned_exec::execute(
            pinned.readable_executable.as_fd(),
            &[std::ffi::CString::new("worker").unwrap()],
        )
        .unwrap();
        panic!("exec неожиданно вернулся");
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("discovery::installed::tests::candidate_readable_fd_executes_pinned_object_after_path_replacement")
            .env(CHILD, "1")
            .stdout(Stdio::null())
            .spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("истёк срок дочернего процесса");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
