use core::fmt;

use carosign_core::{
    device::DeviceEpoch,
    ids::{CommandId, MutationId, WorkerInstanceId},
};

/// Reason to issue permission to send.
#[derive(Clone, PartialEq, Eq)]
pub struct ArmMutation {
    pub mutation: MutationId,
    /// An worker who will be sending.
    pub worker: WorkerInstanceId,
    /// The age of the device.
    pub epoch: DeviceEpoch,
    /// The number of the command we're allowed to send.
    pub command: CommandId,
    /// Restore link if the provider has given it in advance (usually not).
    pub recovery_ref: Option<carosign_core::enrollment::OpaqueProviderRef>,
}

impl fmt::Debug for ArmMutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArmMutation")
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}
