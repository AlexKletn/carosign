//! The composition of the system check of the installed package provider.
//!
//! The parsing is done by the existing runtime contract, and the lack of a source of origin
//! in a Linux adapter remains a failure, not an empty admission.

use std::os::fd::{AsFd, OwnedFd};

use carosign_core::ids::{PackageId, ProfileId, ProviderId};
use carosign_linux::discovery::launch::{
    InstallationMetadataDecoder, LaunchRefusal, MetadataDecodeError, ParsedInstallationManifest,
    PreparedInstallation,
};
use carosign_runtime::provider::packages::{parse_manifest, parse_profile};

/// The parent directories and database file must belong to the system owner.
/// Otherwise, a non-privileged process could replace the path before opening redb.
pub fn trusted_store_path(path: &std::path::Path) -> Result<(), QuarantineCommandError> {
    use std::os::unix::fs::MetadataExt;
    use std::path::{Component, PathBuf};

    let mut current = PathBuf::from("/");
    let root =
        std::fs::symlink_metadata(&current).map_err(|_| QuarantineCommandError::Unavailable)?;
    if !root.is_dir() || root.uid() != 0 || root.mode() & 0o022 != 0 {
        return Err(QuarantineCommandError::Refused);
    }
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return Err(QuarantineCommandError::Refused);
    }
    for component in components {
        let Component::Normal(name) = component else {
            return Err(QuarantineCommandError::Refused);
        };
        current.push(name);
        let meta =
            std::fs::symlink_metadata(&current).map_err(|_| QuarantineCommandError::Unavailable)?;
        if meta.file_type().is_symlink() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err(QuarantineCommandError::Refused);
        }
    }
    Ok(())
}

/// Performs quarantine installation in a separate privileged process.
///
/// Argument order: base, signature policy, supervisor policy, archive, signature, keyring,
/// gpgv, installation root, staging, relative version name, then pairs: file path in isolated
/// runtime and absolute source path.
/// Root and one thread are required; successful fixation remains quarantined.
pub fn quarantine_command(args: &[std::ffi::OsString]) -> Result<(), QuarantineCommandError> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;

    if args.len() < 10 || !(args.len() - 10).is_multiple_of(2) {
        return Err(QuarantineCommandError::Refused);
    }
    // Both UIDs are checked: the setuid program does not become an administrative interface
    // for the unprivileged caller.
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(QuarantineCommandError::Refused);
    }
    let tasks = std::fs::read_dir("/proc/self/task")
        .map_err(|_| QuarantineCommandError::Refused)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| QuarantineCommandError::Refused)?;
    if tasks.len() != 1 {
        return Err(QuarantineCommandError::Refused);
    }
    let absolute = |index: usize| -> Result<&Path, QuarantineCommandError> {
        let path = Path::new(&args[index]);
        if !path.is_absolute() {
            return Err(QuarantineCommandError::Refused);
        }
        Ok(path)
    };
    for index in 0..9 {
        absolute(index)?;
    }
    let open = |path: &Path, directory: bool| -> Result<std::fs::File, QuarantineCommandError> {
        let flags = libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if directory { libc::O_DIRECTORY } else { 0 };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(path)
            .map_err(|_| QuarantineCommandError::Unavailable)?;
        let meta = file
            .metadata()
            .map_err(|_| QuarantineCommandError::Unavailable)?;
        if (directory && !meta.is_dir()) || (!directory && !meta.is_file()) {
            return Err(QuarantineCommandError::Refused);
        }
        Ok(file)
    };
    let artifact = open(absolute(3)?, false)?;
    let signature = open(absolute(4)?, false)?;
    let keyring = open(absolute(5)?, false)?;
    let verifier = open(absolute(6)?, false)?;
    let root = open(absolute(7)?, true)?;
    let staging = open(absolute(8)?, true)?;
    let version_dir = args[9].to_str().ok_or(QuarantineCommandError::Refused)?;
    let mut runtime = Vec::new();
    for pair in args[10..].chunks_exact(2) {
        let name = pair[0].to_str().ok_or(QuarantineCommandError::Refused)?;
        let path = Path::new(&pair[1]);
        if !path.is_absolute() {
            return Err(QuarantineCommandError::Refused);
        }
        runtime.push((name, open(path, false)?));
    }
    let borrowed: Vec<_> = runtime
        .iter()
        .map(|(name, file)| (*name, file.as_fd()))
        .collect();
    trusted_store_path(absolute(0)?)?;
    let store = carosign_persistence::redb::backend::RedbStore::open_current_guarded(absolute(0)?)
        .map_err(|_| QuarantineCommandError::Unavailable)?;
    let inputs = carosign_linux::discovery::publication_owner::QuarantineInputs {
        signature_policy_path: absolute(1)?,
        supervisor_policy_path: absolute(2)?,
        artifact: artifact.as_fd(),
        signature: signature.as_fd(),
        keyring: keyring.as_fd(),
        verifier: verifier.as_fd(),
        runtime: &borrowed,
        decoder: &RuntimeDecoder,
        installation_root: root.as_fd(),
        staging: staging.as_fd(),
        version_dir,
        timeout: std::time::Duration::from_secs(30),
    };
    // SAFETY: The UID and single-threaded execution were checked. `bind_location` independently
    // verifies root ownership of the root, staging directory, and all path components.
    match unsafe { crate::install_quarantine::prepare_and_commit(inputs, &store) } {
        Ok(()) => Ok(()),
        Err(crate::install_quarantine::QuarantineFailure::Storage(
            carosign_persistence::installation_records::PublicationFailure::RecoveryRequired,
        )) => Err(QuarantineCommandError::RecoveryRequired),
        Err(_) => Err(QuarantineCommandError::Unavailable),
    }
}

/// Verifies the installed package through the system source and the Linux kernel.
/// The success of this verification does not yet constitute a permit to perform by the
/// provider.
pub fn prepare(
    root: OwnedFd,
    package: PackageId,
    provider: ProviderId,
    profile: ProfileId,
) -> Result<PreparedInstallation, LaunchRefusal> {
    carosign_linux::discovery::launch::prepare_installed(
        root,
        package,
        provider,
        profile,
        &RuntimeDecoder,
    )
}

#[path = "installation.tests.rs"]
#[cfg(test)]
mod tests;

mod runtime_decoder;
pub(crate) use runtime_decoder::RuntimeDecoder;
mod quarantine_command_error;
pub use quarantine_command_error::QuarantineCommandError;
