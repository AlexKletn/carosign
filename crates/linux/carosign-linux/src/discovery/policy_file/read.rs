use super::PolicyReadError;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

use crate::sys::{self, LaunchMetadata};

/// A time limit for internal reading; it is not a wire contract.
const MAX_READ: usize = 1024 * 1024;

fn protected(meta: &LaunchMetadata, owner: u32, directory: bool) -> bool {
    meta.identity.uid == owner
        && meta.mode & (libc::S_IWGRP | libc::S_IWOTH) == 0
        && meta.mode & libc::S_IFMT
            == if directory {
                libc::S_IFDIR
            } else {
                libc::S_IFREG
            }
}

/// Reads nothing more. `limit` bytes without re-searching the file by name.
/// The root must be opened by the trusted owner of the installation; there is no proof of the
/// absence of a privileged writer and the synchronization of updates.
pub(in crate::discovery) fn read(
    root: &OwnedFd,
    path: &Path,
    expected_owner: u32,
    limit: usize,
) -> Result<Vec<u8>, PolicyReadError> {
    if limit == 0 || limit > MAX_READ {
        return Err(PolicyReadError::InvalidLimit);
    }
    let raw = path.as_os_str().as_bytes();
    if raw.is_empty()
        || raw
            .split(|b| *b == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b"..")
    {
        return Err(PolicyReadError::InvalidPath);
    }
    let components: Vec<_> = path.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(PolicyReadError::InvalidPath);
    }
    let metadata = sys::launch_metadata(root).map_err(|e| PolicyReadError::Kernel(e.errno))?;
    if !protected(&metadata, expected_owner, true) {
        return Err(PolicyReadError::UnsafeObject);
    }
    let mut current = None;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            unreachable!()
        };
        let opened = sys::open_launch_component(current.as_ref().unwrap_or(root), name)
            .map_err(|e| PolicyReadError::Kernel(e.errno))?;
        let metadata =
            sys::launch_metadata(&opened).map_err(|e| PolicyReadError::Kernel(e.errno))?;
        if !protected(&metadata, expected_owner, index + 1 != components.len()) {
            return Err(PolicyReadError::UnsafeObject);
        }
        current = Some(opened);
    }
    let pinned = current.ok_or(PolicyReadError::InvalidPath)?;
    let before = sys::launch_metadata(&pinned).map_err(|e| PolicyReadError::Kernel(e.errno))?;
    let readable =
        sys::reopen_pinned_regular(&pinned).map_err(|e| PolicyReadError::Kernel(e.errno))?;
    if sys::launch_metadata(&readable).map_err(|e| PolicyReadError::Kernel(e.errno))? != before {
        return Err(PolicyReadError::Changed);
    }
    let file: std::fs::File = readable.into();
    let first = file
        .metadata()
        .map_err(|e| PolicyReadError::Kernel(e.raw_os_error().unwrap_or(libc::EIO)))?;
    if first.size() > limit as u64 {
        return Err(PolicyReadError::TooLarge);
    }
    let mut bytes = Vec::new();
    (&file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| PolicyReadError::Kernel(e.raw_os_error().unwrap_or(libc::EIO)))?;
    if bytes.len() > limit {
        return Err(PolicyReadError::TooLarge);
    }
    let last = file
        .metadata()
        .map_err(|e| PolicyReadError::Kernel(e.raw_os_error().unwrap_or(libc::EIO)))?;
    if (
        first.dev(),
        first.ino(),
        first.mode(),
        first.uid(),
        first.gid(),
        first.size(),
        first.mtime(),
        first.mtime_nsec(),
        first.ctime(),
        first.ctime_nsec(),
    ) != (
        last.dev(),
        last.ino(),
        last.mode(),
        last.uid(),
        last.gid(),
        last.size(),
        last.mtime(),
        last.mtime_nsec(),
        last.ctime(),
        last.ctime_nsec(),
    ) || sys::launch_metadata(&pinned).map_err(|e| PolicyReadError::Kernel(e.errno))? != before
    {
        return Err(PolicyReadError::Changed);
    }
    Ok(bytes)
}
