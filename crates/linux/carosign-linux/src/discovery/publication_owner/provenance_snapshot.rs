use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use super::directory_metadata::directory_metadata;

// These images are only intended to re-check the signature when quarantine is restored. Their
// existence is not a launch authorization.
pub(super) const PROVENANCE_ARTIFACT: &std::ffi::CStr = c".carosign-origin-artifact";
pub(super) const PROVENANCE_SIGNATURE: &std::ffi::CStr = c".carosign-origin-signature";

/// Retained materials for re-examination; measurements do not certify signature and do not
/// give the right to quarantine.
pub(super) struct ReopenedProvenance {
    pub(super) artifact: OwnedFd,
    pub(super) signature: OwnedFd,
}

/// Opens only the saved files from the retained directory and checks the measurements from
/// the quarantine image, confirms the directory and re-verifies the signature and independent
/// policies remain with the owner.
#[allow(dead_code)]
pub(super) fn reopen_provenance(
    root: BorrowedFd<'_>,
    owner: u32,
    expected: &[[u8; 32]; 2],
) -> io::Result<ReopenedProvenance> {
    reopen_provenance_with(root, owner, expected, |fd, digest| {
        crate::discovery::verity::matches_approved_digest(fd, digest)
    })
}

fn reopen_provenance_with(
    root: BorrowedFd<'_>,
    owner: u32,
    expected: &[[u8; 32]; 2],
    measure: impl Fn(BorrowedFd<'_>, &[u8; 32]) -> io::Result<bool>,
) -> io::Result<ReopenedProvenance> {
    directory_metadata(root, owner)?;
    let mut files = Vec::with_capacity(2);
    for (index, (name, max)) in [
        (PROVENANCE_ARTIFACT, 256 * 1024 * 1024u64),
        (PROVENANCE_SIGNATURE, 1024 * 1024u64),
    ]
    .into_iter()
    .enumerate()
    {
        if expected[index] == [0; 32] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid provenance digest",
            ));
        }
        // SAFETY: The name is fixed and contains no separators; the root is held.
        let raw = unsafe {
            libc::openat(
                root.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG
            || stat.st_uid != owner
            || stat.st_mode & (libc::S_IWGRP | libc::S_IWOTH) != 0
            || stat.st_size <= 0
            || stat.st_size as u64 > max
            || !measure(fd.as_fd(), &expected[index])?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid provenance file",
            ));
        }
        files.push(fd);
    }
    let mut files = files.into_iter();
    Ok(ReopenedProvenance {
        artifact: files.next().unwrap(),
        signature: files.next().unwrap(),
    })
}

#[cfg(test)]
#[path = "provenance_snapshot.tests.rs"]
mod tests;
