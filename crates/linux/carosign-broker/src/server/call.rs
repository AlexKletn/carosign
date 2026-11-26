use carosign_core::device::DeviceEpoch;
use carosign_core::ids::CommandId;
use carosign_core::ids::MutationId;
use carosign_core::ids::OperationId;
use carosign_core::ids::PackageId;
use carosign_core::ids::ResourceId;

/// A call to the worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    /// Handler.
    pub operation_ref: [u8; 16],
    /// Package (from the anchor, not from the text).
    pub package: PackageId,
    /// Resource (from the peg).
    pub resource: ResourceId,
    /// It's the time of the call.
    pub epoch: DeviceEpoch,
    pub operation: Option<OperationId>,
    /// Changes (for changers)
    pub mutation: Option<MutationId>,
    /// Command (for changers).
    pub command: Option<CommandId>,
}
