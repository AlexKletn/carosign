use super::ApprovedPolicy;
use super::InstallationError;
use super::InstallationKey;
use super::PolicyAuthority;

/// Without a working system installer, confirmation is not available.
struct UnavailablePolicyAuthority;

impl PolicyAuthority for UnavailablePolicyAuthority {
    fn approved_policy(&self, _: InstallationKey) -> Result<ApprovedPolicy, InstallationError> {
        Err(InstallationError::Unavailable)
    }
}
