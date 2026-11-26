use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{FileExt, MetadataExt};

use super::duplicate_fd::duplicate_fd;

/// It saves a limited sealed photo under a reserved name.
/// A partially prepared catalogue is completely discarded by the caller.
pub(super) fn stage_provenance(
    root: BorrowedFd<'_>,
    name: &std::ffi::CStr,
    source: BorrowedFd<'_>,
    max: u64,
    measure: impl FnOnce(BorrowedFd<'_>) -> io::Result<[u8; 32]>,
) -> io::Result<(OwnedFd, [u8; 32])> {
    let source = std::fs::File::from(duplicate_fd(source)?);
    let len = source.metadata()?.len();
    if len == 0 || len > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "origin size"));
    }
    let raw = unsafe {
        libc::openat(
            root.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut written = unsafe { std::fs::File::from_raw_fd(raw) };
    let mut offset = 0u64;
    let mut buffer = [0u8; 16384];
    while offset < len {
        let wanted = ((len - offset) as usize).min(buffer.len());
        let count = source.read_at(&mut buffer[..wanted], offset)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "origin truncated",
            ));
        }
        written.write_all(&buffer[..count])?;
        offset += count as u64;
    }
    written.sync_all()?;
    let original = written.metadata()?;
    if unsafe { libc::fsync(root.as_raw_fd()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let raw = unsafe {
        libc::openat(
            root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let result = unsafe { std::fs::File::from_raw_fd(raw) };
    let actual = result.metadata()?;
    if original.dev() != actual.dev() || original.ino() != actual.ino() || actual.len() != len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "origin replaced",
        ));
    }
    drop(written);
    let digest = measure(result.as_fd())?;
    // We compare an already immutable object with a sealed image, not just an inode.
    let mut other = [0u8; 16384];
    let mut offset = 0u64;
    while offset < len {
        let wanted = ((len - offset) as usize).min(buffer.len());
        let a = source.read_at(&mut buffer[..wanted], offset)?;
        let b = result.read_at(&mut other[..wanted], offset)?;
        if a == 0 || a != b || buffer[..a] != other[..b] {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "origin changed"));
        }
        offset += a as u64;
    }
    result.sync_all()?;
    Ok((result.into(), digest))
}
