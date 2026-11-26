use std::io;

use super::*;
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn cleanup_in_separate_process() {
    const CHILD: &str = "CAROSIGN_FD_CLEANUP_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let file = std::fs::File::open("/dev/null").unwrap();
        // SAFETY: fcntl creates a new descriptor with a specified lower bound.
        let high = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 1024) };
        assert!(high >= 1024);
        // SAFETY: A separate permitted descriptor is created by the kernel.
        let inherited = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        assert!((10..1024).contains(&inherited));
        let allowed = [
            AllowedFd {
                fd: 0,
                disposition: ExecDisposition::Inherit,
            },
            AllowedFd {
                fd: 1,
                disposition: ExecDisposition::Inherit,
            },
            AllowedFd {
                fd: 2,
                disposition: ExecDisposition::Inherit,
            },
            AllowedFd {
                fd: file.as_raw_fd(),
                disposition: ExecDisposition::Close,
            },
            AllowedFd {
                fd: inherited,
                disposition: ExecDisposition::Inherit,
            },
        ];
        retain_only(&allowed).unwrap();
        // SAFETY: F_GETFD It doesn't change the descriptor; the high is already closed.
        assert_eq!(unsafe { libc::fcntl(high, libc::F_GETFD) }, -1);
        // SAFETY: Checking the flags of the saved descriptor.
        assert_ne!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        assert!(unsafe { libc::fcntl(1, libc::F_GETFD) } >= 0);
        let status = Command::new("/bin/sh")
                .args([
                    "-c",
                    "test -e /proc/self/fd/\"$1\" && test ! -e /proc/self/fd/\"$2\" && test ! -e /proc/self/fd/\"$3\"",
                    "fd-check",
                    &inherited.to_string(),
                    &file.as_raw_fd().to_string(),
                    &high.to_string(),
                ])
                .status()
                .unwrap();
        assert!(status.success());
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process::fd_cleanup::tests::cleanup_in_separate_process",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("время испытания очистки FD истекло");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn duplicate_and_invalid_descriptors_are_rejected_before_mutation() {
    let repeated = AllowedFd {
        fd: 1,
        disposition: ExecDisposition::Inherit,
    };
    assert_eq!(
        retain_only(&[repeated, repeated]).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    let invalid = AllowedFd {
        fd: -1,
        disposition: ExecDisposition::Inherit,
    };
    assert_eq!(
        retain_only(&[invalid]).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}
