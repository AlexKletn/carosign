use super::ApprovedPolicy;
use super::InstallationError;
use super::InstallationKey;

/// A source of system confirmation, not client text.
pub(super) trait PolicyAuthority {
    fn approved_policy(&self, key: InstallationKey) -> Result<ApprovedPolicy, InstallationError>;
}
