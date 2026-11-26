use super::InstallationAuthority;
use super::InstallationError;
use super::InstallationKey;
use super::PinnedCandidate;
use crate::sys;
use crate::sys::LaunchMetadata;
use std::os::fd::OwnedFd;
use std::path::Component;

/// It compares the record and reveals its relative path from the retained root.
/// The root is passed on by the owner of the installation and is not considered trusted in
/// itself.
pub(super) fn pin_candidate(
    authority: &dyn InstallationAuthority,
    key: InstallationKey,
    root: OwnedFd,
    expected_owner: u32,
) -> Result<PinnedCandidate, InstallationError> {
    let record = authority.approved(key)?;
    if record.key != key {
        return Err(InstallationError::MismatchedRecord);
    }
    let components: Vec<_> = record.executable.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(InstallationError::InvalidPath);
    }
    let root_metadata =
        sys::launch_metadata(&root).map_err(|e| InstallationError::Kernel(e.errno))?;
    if !protected(&root_metadata, expected_owner, true) {
        return Err(InstallationError::UnsafeObject);
    }
    if root_metadata.identity != record.root_identity {
        return Err(InstallationError::Replaced);
    }
    let mut current = None;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            unreachable!()
        };
        let parent = current.as_ref().unwrap_or(&root);
        let next = sys::open_launch_component(parent, name)
            .map_err(|e| InstallationError::Kernel(e.errno))?;
        let metadata =
            sys::launch_metadata(&next).map_err(|e| InstallationError::Kernel(e.errno))?;
        let directory = index + 1 != components.len();
        if !protected(&metadata, expected_owner, directory) {
            return Err(InstallationError::UnsafeObject);
        }
        if !directory && metadata.identity != record.executable_identity {
            return Err(InstallationError::Replaced);
        }
        current = Some(next);
    }
    let executable = current.ok_or(InstallationError::InvalidPath)?;
    let pinned_metadata =
        sys::launch_metadata(&executable).map_err(|e| InstallationError::Kernel(e.errno))?;
    if !protected(&pinned_metadata, expected_owner, false)
        || pinned_metadata.identity != record.executable_identity
    {
        return Err(InstallationError::Replaced);
    }
    let readable_executable =
        sys::reopen_pinned_regular(&executable).map_err(|e| InstallationError::Kernel(e.errno))?;
    let readable_metadata = sys::launch_metadata(&readable_executable)
        .map_err(|e| InstallationError::Kernel(e.errno))?;
    let after_metadata =
        sys::launch_metadata(&executable).map_err(|e| InstallationError::Kernel(e.errno))?;
    if readable_metadata != pinned_metadata || after_metadata != pinned_metadata {
        return Err(InstallationError::Replaced);
    }
    Ok(PinnedCandidate {
        _root: root,
        _executable: executable,
        readable_executable,
        identity: record.executable_identity,
    })
}

fn protected(metadata: &LaunchMetadata, owner: u32, directory: bool) -> bool {
    let kind = metadata.mode & libc::S_IFMT;
    metadata.identity.uid == owner
        && metadata.mode & (libc::S_IWGRP | libc::S_IWOTH) == 0
        && if directory {
            kind == libc::S_IFDIR
        } else {
            kind == libc::S_IFREG && metadata.mode & 0o111 != 0
        }
}
