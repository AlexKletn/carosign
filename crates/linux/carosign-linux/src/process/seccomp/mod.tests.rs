use super::install::MAX_CALLS;
use super::*;
use std::io;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn rejects_invalid_policy_before_installation() {
    assert!(matches!(install(&[]), Err(FilterError::InvalidPolicy)));
    assert!(matches!(install(&[-1]), Err(FilterError::InvalidPolicy)));
    assert!(matches!(
        install(&[0; MAX_CALLS + 1]),
        Err(FilterError::InvalidPolicy)
    ));
    assert!(matches!(
        install(&[0x4000_0000]),
        Err(FilterError::InvalidPolicy)
    ));
    for forbidden in [
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
    ] {
        assert!(matches!(
            install(&[libc::SYS_getpid, forbidden]),
            Err(FilterError::InvalidPolicy)
        ));
    }
}

#[test]
fn allowlist_is_enforced_in_separate_process() {
    const CHILD: &str = "CAROSIGN_SECCOMP_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: We limit only a single test process.
        if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
            unsafe { libc::_exit(10) };
        }
        if install(&[libc::SYS_getpid, libc::SYS_exit_group]).is_err() {
            unsafe { libc::_exit(11) };
        }
        // After installing the filter, we use only authorized calls.
        let ok = unsafe { libc::syscall(libc::SYS_getpid) } > 0;
        // SAFETY: Check the kernel failure on an unresolved call.
        let denied = unsafe { libc::syscall(libc::SYS_getppid) } == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
        // SAFETY: System calls without permission are checked before they are executed by the
        // filter; no socket or ring is created.
        let no_socket =
            unsafe { libc::syscall(libc::SYS_socket, libc::AF_UNIX, libc::SOCK_STREAM, 0) } == -1
                && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
        let no_ring = unsafe {
            libc::syscall(
                libc::SYS_io_uring_setup,
                0,
                std::ptr::null::<libc::c_void>(),
            )
        } == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
        unsafe {
            libc::_exit(if ok && denied && no_socket && no_ring {
                0
            } else {
                12
            })
        };
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process::seccomp::tests::allowlist_is_enforced_in_separate_process",
        ])
        .env(CHILD, "1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "фильтр: {status}");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("срок тестового процесса истёк");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
