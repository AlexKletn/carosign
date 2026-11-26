use super::MountError;
use super::MountInput;
use super::MountStage;
use super::clone_source;
use std::ffi::CString;
use std::io;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

const MOVE_MOUNT_F_EMPTY_PATH: u32 = 4;

const MOVE_MOUNT_T_EMPTY_PATH: u32 = 0x40;

pub(super) const MOUNT_ATTR_RDONLY: u64 = 1;

pub(super) const MOUNT_ATTR_NOSUID: u64 = 2;

pub(super) const MOUNT_ATTR_NODEV: u64 = 4;

pub(super) fn error(stage: MountStage) -> MountError {
    MountError {
        stage,
        source: io::Error::last_os_error(),
    }
}

pub(crate) fn attach_tree_for_identity(
    root: BorrowedFd<'_>,
    name: &str,
    tree: BorrowedFd<'_>,
    uid: libc::uid_t,
    gid: libc::gid_t,
) -> Result<(), MountError> {
    if uid == 0 || gid == 0 || uid == libc::uid_t::MAX || gid == libc::gid_t::MAX {
        return Err(MountError {
            stage: MountStage::RestrictDirectory,
            source: io::ErrorKind::InvalidInput.into(),
        });
    }
    attach_tree_inner(root, name, MountInput::Tree(tree), Some((uid, gid)))
}

pub(super) fn attach_tree_inner(
    root: BorrowedFd<'_>,
    name: &str,
    input: MountInput<'_>,
    identity: Option<(libc::uid_t, libc::gid_t)>,
) -> Result<(), MountError> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.iter().any(|part| {
        part.is_empty()
            || *part == "."
            || *part == ".."
            || part.len() > 255
            || part.as_bytes().contains(&0)
    }) {
        return Err(MountError {
            stage: MountStage::TargetName,
            source: io::ErrorKind::InvalidInput.into(),
        });
    }
    let name =
        CString::new(*parts.last().expect("split возвращает компоненту")).expect("NUL проверен");
    if let MountInput::Source(source) = &input {
        // SAFETY: The kernel fills in the stat for the retained source.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(source.as_raw_fd(), &mut stat) } != 0 {
            return Err(error(MountStage::InspectSource));
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(MountError {
                stage: MountStage::InspectSource,
                source: io::ErrorKind::InvalidInput.into(),
            });
        }
    }
    let mut directory: Option<OwnedFd> = None;
    for part in &parts[..parts.len() - 1] {
        let component = CString::new(*part).expect("NUL проверен");
        let parent = directory
            .as_ref()
            .map_or(root.as_raw_fd(), AsRawFd::as_raw_fd);
        // SAFETY: Name is one verified component of the retained directory.
        if unsafe { libc::mkdirat(parent, component.as_ptr(), 0o700) } != 0 {
            let source = io::Error::last_os_error();
            if source.raw_os_error() != Some(libc::EEXIST) {
                return Err(MountError {
                    stage: MountStage::CreateDirectory,
                    source,
                });
            }
        }
        // SAFETY: O_NOFOLLOW prohibits symbolic reference, O_DIRECTORY - different types.
        let fd = unsafe {
            libc::openat(
                parent,
                component.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(error(MountStage::OpenDirectory));
        }
        // SAFETY: The successful openat has returned a new FD.
        directory = Some(unsafe { OwnedFd::from_raw_fd(fd) });
        if let Some((_uid, gid)) = identity {
            // SAFETY: fd holds the verified directory, the name is not reopened.
            if unsafe { libc::fchown(fd, 0, gid) } != 0 || unsafe { libc::fchmod(fd, 0o510) } != 0 {
                return Err(error(MountStage::RestrictDirectory));
            }
        }
    }
    let parent = directory
        .as_ref()
        .map_or(root.as_raw_fd(), AsRawFd::as_raw_fd);
    // SAFETY: One verified name is created only in the retained directory.
    let target = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_CREAT | libc::O_EXCL | libc::O_WRONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if target < 0 {
        return Err(error(MountStage::CreateTarget));
    }
    // SAFETY: Openat passed on a new owned descriptor.
    let target = unsafe { OwnedFd::from_raw_fd(target) };
    let cloned;
    let tree = match input {
        MountInput::Source(source) => {
            cloned = clone_source(source)?;
            cloned.as_fd()
        }
        MountInput::Tree(tree) => tree,
    };
    // SAFETY: The tree and target are held by FD; the name is not re-allowed.
    if unsafe {
        libc::syscall(
            libc::SYS_move_mount,
            tree.as_raw_fd(),
            c"".as_ptr(),
            target.as_raw_fd(),
            c"".as_ptr(),
            MOVE_MOUNT_F_EMPTY_PATH | MOVE_MOUNT_T_EMPTY_PATH,
        )
    } != 0
    {
        return Err(error(MountStage::Attach));
    }
    Ok(())
}
