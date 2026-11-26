use core::fmt;

use carosign_core::ids::{PackageId, ProfileId, ProviderId};

/// Package record (for idempotence and reconciliation).
#[derive(Clone, PartialEq, Eq)]
pub struct PackageBinding {
    /// The name of the package.
    pub name: carosign_core::provider::PackageName,
    /// Packet ID.
    pub package: PackageId,
    /// Family.
    pub provider: ProviderId,
    pub profiles: Vec<(String, ProfileId)>,
}

impl fmt::Debug for PackageBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageBinding")
            .field("name", &self.name)
            .field("profiles", &self.profiles.len())
            .finish_non_exhaustive()
    }
}
