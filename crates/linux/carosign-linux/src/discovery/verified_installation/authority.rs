use super::{ApprovedInstallation, InstallationKey, Refusal};

pub(super) trait Authority {
    fn approved(&self, key: InstallationKey) -> Result<ApprovedInstallation, Refusal>;
    /// Recheck before transferring the result to the owner of the launch.
    fn revalidate(&self, record: &ApprovedInstallation) -> Result<(), Refusal>;
}
