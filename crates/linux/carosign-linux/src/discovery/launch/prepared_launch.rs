use super::LaunchRefusal;
use crate::process::verified_exec::LaunchError;
use crate::sys;
use crate::sys::FileIdentity;
use std::os::fd::OwnedFd;

/// That's verified and open.
#[derive(Debug)]
pub struct PreparedLaunch {
    /// Open file; `exec` It's going down it, not down the road.
    pub executable: OwnedFd,
    /// That's been verified.
    pub identity: FileIdentity,
}

/// Preparing launch.
///
/// The file is fixed as `O_PATH`So opening an inappropriate FIFO or device doesn't perform
/// their usual opening. There's no kernel error bypass.
pub fn prepare(path: &str) -> Result<PreparedLaunch, LaunchRefusal> {
    let executable =
        sys::open_launch_candidate(path).map_err(|error| LaunchRefusal::Kernel(error.errno))?;
    let pinned =
        sys::launch_metadata(&executable).map_err(|error| LaunchRefusal::Kernel(error.errno))?;
    inspect_root_owned_path(path, pinned)?;
    // The old interface does not receive system installation identifiers and cannot request
    // confirmation; its previous failure is saved.
    Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot))
}

/// Verifying DAC metadata is just one barrier, not trusting the package.
pub(super) fn root_owned_protected(metadata: &sys::LaunchMetadata, directory: bool) -> bool {
    let kind = metadata.mode & libc::S_IFMT;
    metadata.identity.uid == 0
        && metadata.mode & (libc::S_IWGRP | libc::S_IWOTH) == 0
        && if directory {
            kind == libc::S_IFDIR
        } else {
            // Without any execution bit, even root won’t run a regular file.
            // This is a necessary condition, not a validation of the rights of the future
            // UID.
            kind == libc::S_IFREG && metadata.mode & 0o111 != 0
        }
}

/// Repeats from `/` And then you check the end object against the end object that's already
/// fixed, and the lack of policy is a failure.
fn inspect_root_owned_path(
    path: &str,
    candidate: sys::LaunchMetadata,
) -> Result<(), LaunchRefusal> {
    use std::path::Component;
    let mut components = std::path::Path::new(path).components();
    if components.next() != Some(Component::RootDir) {
        return Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot));
    }
    let mut current =
        sys::open_launch_candidate("/").map_err(|error| LaunchRefusal::Kernel(error.errno))?;
    let root =
        sys::launch_metadata(&current).map_err(|error| LaunchRefusal::Kernel(error.errno))?;
    if !root_owned_protected(&root, true) {
        return Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot));
    }
    let mut components = components.peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot));
        };
        let next = sys::open_launch_component(&current, name)
            .map_err(|error| LaunchRefusal::Kernel(error.errno))?;
        let metadata =
            sys::launch_metadata(&next).map_err(|error| LaunchRefusal::Kernel(error.errno))?;
        let directory = components.peek().is_some();
        if !root_owned_protected(&metadata, directory) {
            return Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot));
        }
        if !directory {
            if metadata != candidate {
                return Err(LaunchRefusal::Replaced);
            }
            return Ok(());
        }
        current = next;
    }
    Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot))
}
