use super::PackageName;
use crate::ids::ProfileId;
use core::fmt;

/// A recognized profile of the device within the package.
#[derive(Clone, PartialEq, Eq)]
pub struct RegisteredProfile {
    /// Canonical profile name.
    pub name: PackageName,
    /// The identifier assigned by the platform.
    pub id: ProfileId,
}

impl fmt::Debug for RegisteredProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredProfile")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
