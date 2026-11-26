use carosign_core::{
    device::DeviceEpoch,
    ids::{CommandId, MutationId, OperationId, WorkerInstanceId},
};

use crate::{enrollment_codec::EnrollmentRecord, mutation_plan::Scope};

pub struct RecoveryEvidence {
    pub mutation: MutationId,
    pub operation: OperationId,
    pub command: CommandId,
    pub worker: WorkerInstanceId,
    pub epoch: DeviceEpoch,
    pub scope: Scope,
    pub digest: [u8; 32],
    pub enrollments: Vec<EnrollmentRecord>,
}
