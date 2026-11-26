use super::InstallationMetadataDecoder;
use super::LaunchRefusal;
use std::os::fd::OwnedFd;

/// Verified installation files that are retained until further supervisor stages.
/// This type does not confirm a sandbox and does not run an worker on its own.
pub struct PreparedInstallation {
    _files: super::super::verified_installation::CheckedFiles,
}

/// Verifies system installation, policy and files without trusting worker data.
/// While the system confirmation source is not connected, the failure returns.
pub fn prepare_installed(
    root: OwnedFd,
    package: carosign_core::ids::PackageId,
    provider: carosign_core::ids::ProviderId,
    profile: carosign_core::ids::ProfileId,
    decoder: &dyn InstallationMetadataDecoder,
) -> Result<PreparedInstallation, LaunchRefusal> {
    let files = super::super::verified_installation::system_validate(
        package, provider, profile, root, decoder,
    )
    .map_err(|error| match error {
        super::super::verified_installation::Refusal::Unavailable => {
            LaunchRefusal::InstallationUnavailable
        }
        _ => LaunchRefusal::InstallationRejected,
    })?;
    Ok(PreparedInstallation { _files: files })
}
