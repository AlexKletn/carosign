use super::ApprovedInstallation;
use super::InstallationAuthority;
use super::InstallationError;
use super::InstallationKey;

/// As long as the installer system adapter is not available, no proof is issued.
pub(super) struct UnavailableInstallationAuthority;

impl InstallationAuthority for UnavailableInstallationAuthority {
    fn approved(&self, _: InstallationKey) -> Result<ApprovedInstallation, InstallationError> {
        Err(InstallationError::Unavailable)
    }
}
