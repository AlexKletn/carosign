use super::MOUNT_ATTR_NODEV;
use super::MOUNT_ATTR_NOSUID;
use super::MOUNT_ATTR_RDONLY;
use super::MountAttr;
use super::MountError;
use super::MountStage;
use super::error;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

// The ABI values of the kernel remain inside the Linux switch.
const OPEN_TREE_CLONE: u32 = 1;

const OPEN_TREE_CLOEXEC: u32 = libc::O_CLOEXEC as u32;

/// Creates a limited copy of the fixed source before changing namespace.
pub(crate) fn clone_source(source: BorrowedFd<'_>) -> Result<OwnedFd, MountError> {
    // SAFETY: The kernel fills the stat for the retained descriptor.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: The source is valid for the call time.
    if unsafe { libc::fstat(source.as_raw_fd(), &mut stat) } != 0 {
        return Err(error(MountStage::InspectSource));
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(MountError {
            stage: MountStage::InspectSource,
            source: io::ErrorKind::InvalidInput.into(),
        });
    }
    // SAFETY: footpath AT_EMPTY_PATH Refers to the retained source.
    let tree = unsafe {
        libc::syscall(
            libc::SYS_open_tree,
            source.as_raw_fd(),
            c"".as_ptr(),
            OPEN_TREE_CLONE | OPEN_TREE_CLOEXEC | libc::AT_EMPTY_PATH as u32,
        )
    };
    if tree < 0 {
        return Err(error(MountStage::CloneSource));
    }
    // SAFETY: successful open_tree Returns a new FD-owned one.
    let tree = unsafe { OwnedFd::from_raw_fd(tree as i32) };
    let attr = MountAttr {
        attr_set: MOUNT_ATTR_RDONLY | MOUNT_ATTR_NOSUID | MOUNT_ATTR_NODEV,
        attr_clr: 0,
        propagation: 0,
        userns_fd: 0,
    };
    // SAFETY: The structure corresponds to ABI, the tree is held by the descriptor.
    if unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            tree.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH,
            &attr,
            std::mem::size_of::<MountAttr>(),
        )
    } != 0
    {
        return Err(error(MountStage::RestrictSource));
    }
    Ok(tree)
}
