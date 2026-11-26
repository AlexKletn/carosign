use std::io;
use std::num::NonZeroU64;

use super::*;
use std::os::fd::AsRawFd;

#[test]
fn detached_tmpfs_is_real_or_reports_kernel_refusal() {
    match create(NonZeroU64::new(1024 * 1024).unwrap()) {
        Ok(root) => {
            // SAFETY: The kernel fills the statfs of the actual mount FD.
            let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
            assert_eq!(unsafe { libc::fstatfs(root.as_raw_fd(), &mut stat) }, 0);
            assert_eq!(stat.f_type as u64, 0x0102_1994);
            assert!(stat.f_blocks * stat.f_bsize as u64 <= 1024 * 1024);
            // SAFETY: Check for the absence of the general /dev directory in the new root.
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
            let flags = unsafe { libc::fcntl(root.as_raw_fd(), libc::F_GETFD) };
            assert_ne!(flags & libc::FD_CLOEXEC, 0);
        }
        Err(refusal) => {
            assert!(refusal.source.raw_os_error().is_some());
            assert!(
                std::env::var_os("CAROSIGN_TMPFS_REQUIRE_SUCCESS").is_none(),
                "tmpfs недоступен: {refusal:?}"
            );
        }
    }
}
