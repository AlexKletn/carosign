use carosign_core::ids::PackageId;
use carosign_core::ids::ProfileId;
use carosign_core::ids::ProviderId;

/// Identifiers of the requested system record, not worker data.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct InstallationKey {
    pub(super) package: PackageId,
    pub(super) provider: ProviderId,
    pub(super) profile: ProfileId,
}
