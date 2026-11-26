use super::RootError;
use super::RootStage;
use super::error;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;

/// Allows passage to a dedicated group without recording and without access to others.
/// The owner remains root so that the worker cannot change the rights of the directory.
/// Call after assembling the tree, before resetting the authority of the helper.
pub(crate) fn assign_owner(
    root: &OwnedFd,
    uid: libc::uid_t,
    gid: libc::gid_t,
) -> Result<(), RootError> {
    if uid == 0 || gid == 0 || uid == libc::uid_t::MAX || gid == libc::gid_t::MAX {
        return Err(RootError {
            stage: RootStage::AssignOwner,
            source: io::ErrorKind::InvalidInput.into(),
        });
    }
    // SAFETY: The empty path indicates the mount FD being held.
    if unsafe { libc::fchownat(root.as_raw_fd(), c"".as_ptr(), 0, gid, libc::AT_EMPTY_PATH) } != 0 {
        return Err(error(RootStage::AssignOwner));
    }
    // libc 0.2.191 does not export this number to aarch64; the value is taken from Linux UAPI
    // asm-generic/unistd.h, ___NR_fchmodat2.
    #[cfg(target_arch = "aarch64")]
    const SYS_FCHMODAT2: libc::c_long = 452;
    #[cfg(not(target_arch = "aarch64"))]
    const SYS_FCHMODAT2: libc::c_long = libc::SYS_fchmodat2;
    // SAFETY: the same object held; the file system name is not allowed again.
    if unsafe {
        libc::syscall(
            SYS_FCHMODAT2,
            root.as_raw_fd(),
            c"".as_ptr(),
            0o510,
            libc::AT_EMPTY_PATH,
        )
    } != 0
    {
        return Err(error(RootStage::RestrictMode));
    }
    Ok(())
}
