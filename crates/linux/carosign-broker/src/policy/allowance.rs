use crate::resources::FdRole;
use crate::resources::ResourceKind;
use carosign_core::ids::PackageId;
use carosign_core::ids::ResourceId;

/// Permitted combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowance {
    pub package: PackageId,
    pub resource: ResourceId,
    pub role: FdRole,
    /// Carrier.
    pub kind: ResourceKind,
}
