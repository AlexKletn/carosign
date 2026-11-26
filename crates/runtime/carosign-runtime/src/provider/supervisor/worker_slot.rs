use super::WorkerHealth;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::WorkerInstanceId;

/// Supervisor's record of one worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSlot {
    /// Launch.
    pub instance: WorkerInstanceId,
    /// The era in which it was launched.
    pub epoch: DeviceEpoch,
    pub health: WorkerHealth,
}
