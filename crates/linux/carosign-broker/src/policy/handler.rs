use carosign_core::ids::PackageId;
use carosign_core::ids::ResourceId;

/// Record of handler `carosign.broker.v1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    /// An identifier that names the worker.
    pub operation_ref: [u8; 16],
    /// The package that's allowed it.
    pub package: PackageId,
    pub resource: ResourceId,
    /// Can it change a stable state?
    pub may_mutate: bool,
    /// The name of the audited processor (non-executable path).
    pub name: &'static str,
}
