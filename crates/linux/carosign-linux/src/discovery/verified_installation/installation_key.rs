use carosign_core::ids::{PackageId, ProfileId, ProviderId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InstallationKey {
    pub(super) package: PackageId,
    pub(super) provider: ProviderId,
    pub(super) profile: ProfileId,
}
