use super::AccessMode;
use crate::ids::ResourceId;
use core::fmt;

/// One requirement: resource and regime.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ResourceClaim {
    /// What a resource.
    pub id: ResourceId,
    /// Which mode you need.
    pub mode: AccessMode,
}

impl fmt::Debug for ResourceClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceClaim")
            .field("id", &"..")
            .field("mode", &self.mode)
            .finish()
    }
}
