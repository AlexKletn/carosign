use carosign_core::device::DeviceEpoch;
use carosign_core::ids::WorkerInstanceId;

/// Reference to the worker and his era: what cuts off other people's messages.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WorkerRef {
    /// Start the process.
    pub instance: WorkerInstanceId,
    /// The era of the device at the time of launch.
    pub device_epoch: DeviceEpoch,
}
