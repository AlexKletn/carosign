use super::fd_role::FdRole;
use super::resource_kind::ResourceKind;
use carosign_core::ids::ResourceId;

/// Resource application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub resource: ResourceId,
    /// Role in extradition.
    pub role: FdRole,
    /// What is this thing (type of media: device, socket, file in memory).
    pub kind: ResourceKind,
}
