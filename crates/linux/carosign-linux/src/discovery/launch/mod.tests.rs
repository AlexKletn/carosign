use crate::process::verified_exec::LaunchError;
use crate::process::verified_exec::Preconditions;
use crate::process::verified_exec::check;
use crate::sys;

use super::*;

fn temporary_directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "carosign-launch-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn plan_does_not_inherit_environment() {
    let plan = LaunchPlan::clean("worker");
    assert_eq!(plan.argv, vec!["worker".to_string()]);
    assert!(!plan.env.iter().any(|item| item.starts_with("LD_")));
}

#[test]
fn missing_file_is_refused_by_kernel_error() {
    assert!(matches!(
        prepare("/nonexistent/definitely-not-here"),
        Err(LaunchRefusal::Kernel(_))
    ));
}

#[test]
fn symlink_cannot_bypass_missing_trust_evidence() {
    assert!(matches!(
        prepare("/proc/self/exe"),
        Err(LaunchRefusal::Kernel(libc::ELOOP))
    ));
}

#[test]
fn candidate_is_pinned_and_symlinks_in_any_component_are_rejected() {
    use std::os::unix::fs::symlink;
    let base = temporary_directory();
    let directory = base.join("package");
    std::fs::create_dir(&directory).unwrap();
    let executable = directory.join("worker");
    std::fs::write(&executable, b"first").unwrap();
    let descriptor = sys::open_launch_candidate(executable.to_str().unwrap()).unwrap();
    let identity = sys::launch_metadata(&descriptor).unwrap().identity;
    assert!(identity.regular);
    std::fs::rename(&executable, directory.join("old-worker")).unwrap();
    std::fs::write(&executable, b"second").unwrap();
    assert_eq!(
        sys::launch_metadata(&descriptor).unwrap().identity,
        identity
    );
    assert_ne!(
        sys::stat(executable.to_str().unwrap()).unwrap().inode,
        identity.inode
    );

    let file_link = directory.join("worker-link");
    symlink("worker", &file_link).unwrap();
    assert_eq!(
        sys::open_launch_candidate(file_link.to_str().unwrap())
            .unwrap_err()
            .errno,
        libc::ELOOP
    );
    let directory_link = base.join("package-link");
    symlink("package", &directory_link).unwrap();
    assert_eq!(
        sys::open_launch_candidate(directory_link.join("worker").to_str().unwrap())
            .unwrap_err()
            .errno,
        libc::ELOOP
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn trusted_path_metadata_rejects_wrong_owner_kind_or_write_access() {
    let base = temporary_directory();
    let file = base.join("worker");
    std::fs::write(&file, b"candidate").unwrap();
    let fd = sys::open_launch_candidate(file.to_str().unwrap()).unwrap();
    let mut metadata = sys::launch_metadata(&fd).unwrap();
    metadata.identity.uid = 0;
    metadata.mode = libc::S_IFREG | 0o755;
    assert!(root_owned_protected(&metadata, false));
    metadata.identity.uid = 1000;
    assert!(!root_owned_protected(&metadata, false));
    metadata.identity.uid = 0;
    for permissions in [0o775, 0o757, 0o777] {
        metadata.mode = libc::S_IFREG | permissions;
        assert!(!root_owned_protected(&metadata, false));
    }
    metadata.mode = libc::S_IFREG | 0o644;
    assert!(!root_owned_protected(&metadata, false));
    metadata.mode = libc::S_IFDIR | 0o755;
    assert!(root_owned_protected(&metadata, true));
    assert!(!root_owned_protected(&metadata, false));
    metadata.mode = libc::S_IFDIR | 0o775;
    assert!(!root_owned_protected(&metadata, true));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn component_open_is_confined_and_rejects_parent_or_link() {
    use std::os::unix::fs::symlink;
    let base = temporary_directory();
    std::fs::write(base.join("worker"), b"candidate").unwrap();
    symlink("worker", base.join("link")).unwrap();
    let root = sys::open_launch_candidate(base.to_str().unwrap()).unwrap();
    assert!(sys::open_launch_component(&root, std::ffi::OsStr::new("worker")).is_ok());
    assert_eq!(
        sys::open_launch_component(&root, std::ffi::OsStr::new("../worker"))
            .unwrap_err()
            .errno,
        libc::EINVAL
    );
    assert_eq!(
        sys::open_launch_component(&root, std::ffi::OsStr::new("link"))
            .unwrap_err()
            .errno,
        libc::ELOOP
    );
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn opening_fifo_candidate_does_not_wait_for_a_writer() {
    let base = temporary_directory();
    let fifo = base.join("worker-fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let path = fifo.to_str().unwrap().to_owned();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = sys::open_launch_candidate(&path)
            .and_then(|fd| sys::launch_metadata(&fd).map(|metadata| metadata.identity));
        let _ = sender.send(result);
    });
    let identity = receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("открытие не должно ждать записи")
        .unwrap();
    assert!(!identity.regular);
    assert!(matches!(
        prepare(fifo.to_str().unwrap()),
        Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot))
    ));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn world_writable_program_is_refused_by_conditions() {
    // ⚠️ We check the rule on properties, not on someone else’s file: you do not need to
    // write anywhere.
    let conditions = Preconditions {
        trusted_root: true,
        regular_file: true,
        root_owned: true,
        not_world_writable: false,
        manifest_ok: true,
    };
    assert_eq!(
        check(&conditions),
        Err(crate::process::verified_exec::LaunchError::WorldWritable)
    );
}
