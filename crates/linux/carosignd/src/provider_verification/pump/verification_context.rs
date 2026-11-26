use super::{CommandId, DeviceEpoch, OperationId, WorkerInstanceId};

/// Context of a verified channel; it is not an authority in itself.
#[derive(Clone, Copy)]
pub(crate) struct VerificationContext {
    pub operation: OperationId,
    pub worker: WorkerInstanceId,
    pub command: CommandId,
    pub authenticator: carosign_core::ids::AuthenticatorId,
    pub epoch: DeviceEpoch,
}
