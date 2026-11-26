use super::ApprovedInstallation;
use super::InstallationError;
use super::InstallationKey;

pub(super) trait InstallationAuthority {
    fn approved(&self, key: InstallationKey) -> Result<ApprovedInstallation, InstallationError>;
}
