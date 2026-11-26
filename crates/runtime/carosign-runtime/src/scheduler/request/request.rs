use super::super::Priority;
use crate::ports::ResourceOwner;
use carosign_core::operation::context::BootInstant;
use carosign_core::resource::ResourceSet;

/// Request in line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Full owner of the issue; in itself is not an authority.
    pub owner: ResourceOwner,
    /// Great.
    pub priority: Priority,
    /// That's right.
    pub set: ResourceSet,
    /// When I came.
    pub since: BootInstant,
    /// Does it change the state (you can never replace the current change)?
    pub mutating: bool,
}
