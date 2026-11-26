use super::Request;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::LeaseId;

/// An application awaiting issuance in the previous era of the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpiredRequest {
    /// Original application to notify its owner.
    pub request: Request,
    /// The number assigned to the application.
    pub lease: LeaseId,
    /// The era at the time of queueing.
    pub epoch: DeviceEpoch,
}
