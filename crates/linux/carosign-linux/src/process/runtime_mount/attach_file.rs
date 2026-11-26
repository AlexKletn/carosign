use super::MountError;
use super::MountInput;
use super::MountStage;
use super::attach_tree_inner;
use std::io;
use std::os::fd::BorrowedFd;

/// Attaches a normal file on the descriptor along a relative path at the root.
/// Only a single helper in its namespace can call this function.
/// The root must be closed to outsiders before the assembly is complete: retaining the FD
/// directory does not prevent another process from moving the directory itself.
/// In case of any refusal, the helper must discard all partially collected root.
pub(crate) fn attach_file(
    root: BorrowedFd<'_>,
    name: &str,
    source: BorrowedFd<'_>,
) -> Result<(), MountError> {
    attach_file_inner(root, name, source, None)
}

/// Collects a tree for a dedicated UID/GID. The directories remain the property of the root;
/// the dedicated group can only pass through, the rest of the access is not available.
/// Helper must maintain exclusive access to the root during assembly.
pub(crate) fn attach_file_for_identity(
    root: BorrowedFd<'_>,
    name: &str,
    source: BorrowedFd<'_>,
    uid: libc::uid_t,
    gid: libc::gid_t,
) -> Result<(), MountError> {
    if uid == 0 || gid == 0 || uid == libc::uid_t::MAX || gid == libc::gid_t::MAX {
        return Err(MountError {
            stage: MountStage::RestrictDirectory,
            source: io::ErrorKind::InvalidInput.into(),
        });
    }
    attach_file_inner(root, name, source, Some((uid, gid)))
}

fn attach_file_inner(
    root: BorrowedFd<'_>,
    name: &str,
    source: BorrowedFd<'_>,
    identity: Option<(libc::uid_t, libc::gid_t)>,
) -> Result<(), MountError> {
    attach_tree_inner(root, name, MountInput::Source(source), identity)
}
