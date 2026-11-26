use super::{ApprovedInstallation, Authority, InstallationKey, Refusal};

pub(super) struct UnavailableAuthority;
impl Authority for UnavailableAuthority {
    fn approved(&self, _: InstallationKey) -> Result<ApprovedInstallation, Refusal> {
        Err(Refusal::Unavailable)
    }
    fn revalidate(&self, _: &ApprovedInstallation) -> Result<(), Refusal> {
        Err(Refusal::Unavailable)
    }
}
