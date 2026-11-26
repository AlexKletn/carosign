use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

pub(super) fn directory_metadata(fd: BorrowedFd<'_>, owner: u32) -> io::Result<libc::stat> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fd is held, the pointer is large enough for stat.
    if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
        || stat.st_uid != owner
        || stat.st_mode & (libc::S_IWGRP | libc::S_IWOTH) != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe directory",
        ));
    }
    Ok(stat)
}
