use std::num::NonZeroU64;
use std::os::fd::{AsFd, OwnedFd};

use super::super::{namespaces, privileges, runtime_mount, tmpfs_root};
use super::{HelperStage, RuntimeFile, RuntimeIdentity};

pub(super) fn prepare_root_with(
    root_size: NonZeroU64,
    identity: RuntimeIdentity,
    runtime: &[RuntimeFile<'_>],
    enter_namespaces: impl FnOnce() -> Result<(), namespaces::NamespaceError>,
    create_root: impl FnOnce(NonZeroU64) -> Result<OwnedFd, tmpfs_root::RootError>,
) -> Result<OwnedFd, (HelperStage, i32)> {
    if let Err(error) = privileges::set_no_new_privileges() {
        return Err((
            HelperStage::NoNewPrivileges,
            error.source.raw_os_error().unwrap_or(libc::EIO),
        ));
    }
    // The clone is created in the original namespace: after unshare, the FD held indicates
    // the mounting of the former namespace and the mounting of the original namespace.
    // open_tree rejects him.
    let trees: Vec<_> = runtime
        .iter()
        .map(|file| {
            runtime_mount::clone_source(file.source).map_err(|error| {
                (
                    HelperStage::RuntimeFiles,
                    error.source.raw_os_error().unwrap_or(libc::EINVAL),
                )
            })
        })
        .collect::<Result<_, _>>()?;
    if let Err(error) = enter_namespaces() {
        return Err((
            HelperStage::Namespaces,
            error.source.raw_os_error().unwrap_or(libc::EIO),
        ));
    }
    let root = match create_root(root_size) {
        Ok(root) => root,
        Err(error) => {
            return Err((
                HelperStage::TemporaryRoot,
                error.source.raw_os_error().unwrap_or(libc::EIO),
            ));
        }
    };
    for (file, tree) in runtime.iter().zip(&trees) {
        if let Err(error) = runtime_mount::attach_tree_for_identity(
            root.as_fd(),
            file.path,
            tree.as_fd(),
            identity.uid,
            identity.gid,
        ) {
            return Err((
                HelperStage::RuntimeFiles,
                error.source.raw_os_error().unwrap_or(libc::EINVAL),
            ));
        }
    }
    // Before the change of identity, the root remains accessible only to the selected group
    // for passage; the worker does not receive the right to change the directory.
    if let Err(error) = tmpfs_root::assign_owner(&root, identity.uid, identity.gid) {
        return Err((
            HelperStage::TemporaryRoot,
            error.source.raw_os_error().unwrap_or(libc::EINVAL),
        ));
    }
    Ok(root)
}
