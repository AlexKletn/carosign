use std::io::{self, Write};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{FileExt, MetadataExt};

/// Creates a separate snapshot of administrative policy in a closed staging.
/// It is not yet an approved recording or launch authorization.
pub(super) fn stage_policy(root: BorrowedFd<'_>, contents: &[u8]) -> io::Result<OwnedFd> {
    if contents.len() > 65_536 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "policy size"));
    }
    // SAFETY: Name is one fixed component, root is retained.
    let raw = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c".carosign-supervisor-policy.toml".as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a new descriptor.
    let mut file = unsafe { std::fs::File::from_raw_fd(raw) };
    file.write_all(contents)?;
    file.sync_all()?;
    let written = file.metadata()?;
    // Name fixation is required separately from content fixation.
    if unsafe { libc::fsync(root.as_raw_fd()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // Open without writing: fs-verity does not turn on when retained write FD.
    let raw = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c".carosign-supervisor-policy.toml".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let reopened = unsafe { std::fs::File::from_raw_fd(raw) };
    let actual = reopened.metadata()?;
    if written.dev() != actual.dev()
        || written.ino() != actual.ino()
        || written.len() != actual.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "policy replaced",
        ));
    }
    drop(file);
    Ok(reopened.into())
}

pub(super) fn policy_matches_snapshot(file: &OwnedFd, contents: &[u8]) -> io::Result<bool> {
    let file = std::fs::File::from(file.try_clone()?);
    if file.metadata()?.len() != contents.len() as u64 {
        return Ok(false);
    }
    let mut offset = 0;
    let mut buf = [0u8; 8192];
    while offset < contents.len() {
        let limit = buf.len().min(contents.len() - offset);
        let count = file.read_at(&mut buf[..limit], offset as u64)?;
        if count == 0 || buf[..count] != contents[offset..offset + count] {
            return Ok(false);
        }
        offset += count;
    }
    Ok(true)
}
