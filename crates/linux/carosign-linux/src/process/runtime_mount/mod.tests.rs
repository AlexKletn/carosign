use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;

use super::*;
use std::os::fd::AsFd;
use std::os::unix::fs::symlink;

fn tempdir() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "carosign-runtime-mount-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn rejects_traversal_and_existing_link() {
    let root = tempdir();
    let source_path = root.join("source");
    std::fs::write(&source_path, b"runtime").unwrap();
    let source = std::fs::File::open(&source_path).unwrap();
    let dir = std::fs::File::open(&root).unwrap();
    for name in ["", ".", "..", "a/../b", "/absolute", "a//b", "a/"] {
        assert_eq!(
            attach_file(dir.as_fd(), name, source.as_fd())
                .unwrap_err()
                .stage,
            MountStage::TargetName
        );
    }
    symlink("source", root.join("link")).unwrap();
    assert_eq!(
        attach_file(dir.as_fd(), "link", source.as_fd())
            .unwrap_err()
            .stage,
        MountStage::CreateTarget
    );
    symlink("/tmp", root.join("escape")).unwrap();
    assert_eq!(
        attach_file(dir.as_fd(), "escape/file", source.as_fd())
            .unwrap_err()
            .stage,
        MountStage::OpenDirectory
    );
    assert_eq!(
        attach_file(dir.as_fd(), "directory", dir.as_fd())
            .unwrap_err()
            .stage,
        MountStage::InspectSource
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_symlink_replacement_cannot_redirect_to_external_directory() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let base = tempdir();
    let root = base.join("root");
    let external = base.join("external");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&external).unwrap();
    std::fs::create_dir(root.join("held")).unwrap();
    let source_path = base.join("source");
    std::fs::write(&source_path, b"runtime").unwrap();
    let source = std::fs::File::open(&source_path).unwrap();
    let dir = std::fs::File::open(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    let thread_root = root.clone();
    let thread_external = external.clone();
    let racer = std::thread::spawn(move || {
        while !signal.load(Ordering::Relaxed) {
            if std::fs::rename(thread_root.join("held"), thread_root.join("entry")).is_ok() {
                let _ = std::fs::rename(thread_root.join("entry"), thread_root.join("held"));
            }
            if symlink(&thread_external, thread_root.join("entry")).is_ok() {
                let _ = std::fs::remove_file(thread_root.join("entry"));
            }
        }
    });
    for i in 0..200 {
        let _ = attach_file(dir.as_fd(), &format!("entry/file{i}"), source.as_fd());
    }
    stop.store(true, Ordering::Relaxed);
    racer.join().unwrap();
    assert_eq!(std::fs::read_dir(&external).unwrap().count(), 0);
    for directory in [root.join("held"), root.join("entry")] {
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.flatten() {
                use std::os::unix::ffi::OsStrExt;
                let path = CString::new(entry.path().as_os_str().as_bytes()).unwrap();
                // SAFETY: The test separates only its own temporary mounting.
                unsafe { libc::umount2(path.as_ptr(), libc::MNT_DETACH) };
            }
        }
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn mounts_file_read_only_inside_detached_root() {
    use std::io::Read;
    use std::num::NonZeroU64;
    let Ok(root) = super::super::tmpfs_root::create(NonZeroU64::new(1024 * 1024).unwrap()) else {
        assert!(
            std::env::var_os("CAROSIGN_RUNTIME_MOUNT_REQUIRE_SUCCESS").is_none(),
            "tmpfs недоступен"
        );
        return;
    };
    let path = tempdir();
    let source_path = path.join("source");
    std::fs::write(&source_path, b"approved runtime").unwrap();
    let source = std::fs::File::open(&source_path).unwrap();
    attach_file(root.as_fd(), "usr/lib/runtime", source.as_fd()).unwrap();
    // SAFETY: The name is static, root is an open directory.
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c"usr/lib/runtime".as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC,
        )
    };
    assert!(fd >= 0);
    // SAFETY: fd is created above and belongs to the test.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut text = String::new();
    file.read_to_string(&mut text).unwrap();
    assert_eq!(text, "approved runtime");
    // SAFETY: Check the failure of the recording through the connected mount.
    let writable = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c"usr/lib/runtime".as_ptr(),
            libc::O_WRONLY | libc::O_CLOEXEC,
        )
    };
    assert_eq!(writable, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EROFS));
    let dev = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c"dev".as_ptr(),
            libc::O_PATH | libc::O_CLOEXEC,
        )
    };
    assert_eq!(dev, -1);
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ENOENT)
    );
    std::fs::remove_dir_all(path).unwrap();
}
