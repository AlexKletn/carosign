use super::PackageName;
use super::PackageVersion;
use crate::ids::PackageId;
use crate::ids::ProviderId;
use core::fmt;

/// What the platform knows about the installed package.
#[derive(Clone, PartialEq, Eq)]
pub struct Package {
    /// A name for people and policy decisions.
    pub name: PackageName,
    /// A stable identifier assigned by the platform.
    pub id: PackageId,
    /// The implementation family.
    pub provider: ProviderId,
    /// Version of the installed package; empty - the package is removed.
    pub installed_version: Option<PackageVersion>,
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Package")
            .field("name", &self.name)
            .field("version", &self.installed_version)
            .finish_non_exhaustive()
    }
}
