use super::super::fd_cleanup;
use super::super::fd_cleanup::AllowedFd;
use super::SwitchError;
use super::SwitchStage;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;

fn last(stage: SwitchStage) -> SwitchError {
    SwitchError {
        stage,
        source: io::Error::last_os_error(),
    }
}

/// In the new mounting space, it changes the root and closes the old FDs.
/// Permitted FDs cannot be directories; otherwise they retain root circumvention.
/// You can’t call out the demon or continue after a mistake.
pub(crate) fn enter(root: OwnedFd, allowed: &[AllowedFd]) -> Result<(), SwitchError> {
    enter_inner(root, allowed, true)
}

/// Changes the root within the namespaces previously created by the assistant.
/// The caller is responsible for the successful creation and privacy of spaces.
pub(crate) fn enter_prepared(root: OwnedFd, allowed: &[AllowedFd]) -> Result<(), SwitchError> {
    enter_inner(root, allowed, false)
}

fn enter_inner(
    root: OwnedFd,
    allowed: &[AllowedFd],
    create_namespaces: bool,
) -> Result<(), SwitchError> {
    // SAFETY: The kernel fills statfs and stats for the open descriptor.
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(root.as_raw_fd(), &mut fs) } != 0
        || unsafe { libc::fstat(root.as_raw_fd(), &mut stat) } != 0
    {
        return Err(last(SwitchStage::CheckRoot));
    }
    if fs.f_type as u64 != 0x0102_1994 || stat.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Err(SwitchError {
            stage: SwitchStage::CheckRoot,
            source: io::ErrorKind::InvalidInput.into(),
        });
    }
    for entry in allowed {
        if entry.fd == root.as_raw_fd() {
            return Err(SwitchError {
                stage: SwitchStage::CheckDescriptors,
                source: io::ErrorKind::InvalidInput.into(),
            });
        }
        let mut item: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: The validity of the descriptor is verified by calling fstat.
        if unsafe { libc::fstat(entry.fd, &mut item) } != 0 {
            return Err(last(SwitchStage::CheckDescriptors));
        }
        if item.st_mode & libc::S_IFMT == libc::S_IFDIR {
            return Err(SwitchError {
                stage: SwitchStage::CheckDescriptors,
                source: io::ErrorKind::InvalidInput.into(),
            });
        }
    }
    if create_namespaces {
        super::super::namespaces::enter().map_err(|error| SwitchError {
            stage: SwitchStage::Namespace,
            source: error.source,
        })?;
    }
    // SAFETY: The root is kept open until the root is changed.
    if unsafe { libc::fchdir(root.as_raw_fd()) } != 0 {
        return Err(last(SwitchStage::ChangeDirectory));
    }
    // SAFETY: The operation is only allowed to a single privileged helper.
    if unsafe { libc::chroot(c".".as_ptr()) } != 0 {
        return Err(last(SwitchStage::ChangeRoot));
    }
    // SAFETY: After chroot, the absolute path signifies a new root.
    if unsafe { libc::chdir(c"/".as_ptr()) } != 0 {
        return Err(last(SwitchStage::LeaveOldDirectory));
    }
    drop(root);
    fd_cleanup::retain_only(allowed).map_err(|source| SwitchError {
        stage: SwitchStage::CloseDescriptors,
        source,
    })?;
    Ok(())
}
