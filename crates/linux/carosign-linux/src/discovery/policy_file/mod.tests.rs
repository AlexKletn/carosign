use super::*;
use crate::sys;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

#[test]
fn bounded_read_rejects_unsafe_paths_files_and_permissions() {
    let root = std::env::temp_dir().join(format!("carosign-policy-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(root.join("policy")).unwrap();
    std::fs::write(root.join("policy/config"), b"abcdef").unwrap();
    let fd = sys::open_launch_candidate(root.to_str().unwrap()).unwrap();
    let owner = sys::launch_metadata(&fd).unwrap().identity.uid;
    assert_eq!(
        read(&fd, Path::new("policy/config"), owner, 6).unwrap(),
        b"abcdef"
    );
    assert_eq!(
        read(&fd, Path::new("policy/config"), owner, 5),
        Err(PolicyReadError::TooLarge)
    );
    assert_eq!(
        read(&fd, Path::new("policy/config"), owner, 0),
        Err(PolicyReadError::InvalidLimit)
    );
    for path in [
        "/policy/config",
        "policy/../config",
        "policy/./config",
        "policy//config",
    ] {
        assert_eq!(
            read(&fd, Path::new(path), owner, 32),
            Err(PolicyReadError::InvalidPath)
        );
    }
    assert_eq!(
        read(&fd, Path::new("policy/config"), owner.wrapping_add(1), 32),
        Err(PolicyReadError::UnsafeObject)
    );
    symlink("config", root.join("policy/link")).unwrap();
    assert!(matches!(
        read(&fd, Path::new("policy/link"), owner, 32),
        Err(PolicyReadError::Kernel(libc::ELOOP))
    ));
    symlink("policy", root.join("linked-dir")).unwrap();
    assert!(matches!(
        read(&fd, Path::new("linked-dir/config"), owner, 32),
        Err(PolicyReadError::Kernel(libc::ELOOP))
    ));
    let fifo = std::ffi::CString::new(root.join("policy/fifo").as_os_str().as_bytes()).unwrap();
    // SAFETY: The path is located in the test catalog.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(
        read(&fd, Path::new("policy/fifo"), owner, 32),
        Err(PolicyReadError::UnsafeObject)
    );
    assert_eq!(
        read(&fd, Path::new("policy"), owner, 32),
        Err(PolicyReadError::UnsafeObject)
    );
    std::fs::set_permissions(
        root.join("policy/config"),
        std::fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    assert_eq!(
        read(&fd, Path::new("policy/config"), owner, 32),
        Err(PolicyReadError::UnsafeObject)
    );
    std::fs::remove_dir_all(root).unwrap();
}
