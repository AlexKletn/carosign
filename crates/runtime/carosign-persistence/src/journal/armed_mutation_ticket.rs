use core::fmt;

use carosign_core::{
    device::DeviceEpoch,
    ids::{CommandId, MutationId, OperationId, WorkerInstanceId},
};

use crate::limits;

/// Ticket for one change of shipment.
///
/// ⚠️ It's only created in the`super::MutationRepository::arm`], and only after the
/// permission has gone on the disk. There is no ticket, there is nothing to send. You can't
/// make a copy.`Clone` Not implemented: sending is allowed exactly one.
pub struct ArmedMutationTicket {
    mutation: MutationId,
    operation: OperationId,
    command: CommandId,
    worker: WorkerInstanceId,
    epoch: DeviceEpoch,
    scope: [u8; limits::SCOPE_LOCK_KEY_LEN],
    effect: Option<carosign_engine::PlannedEffect>,
}

impl ArmedMutationTicket {
    /// Issuing a ticket. `pub(crate)`You can't create it outside of the crate.
    pub(crate) fn new(
        mutation: MutationId,
        operation: OperationId,
        command: CommandId,
        worker: WorkerInstanceId,
        epoch: DeviceEpoch,
        scope: [u8; limits::SCOPE_LOCK_KEY_LEN],
        effect: Option<carosign_engine::PlannedEffect>,
    ) -> Self {
        Self {
            mutation,
            operation,
            command,
            worker,
            epoch,
            scope,
            effect,
        }
    }

    /// Change.
    pub fn mutation(&self) -> MutationId {
        self.mutation
    }
    /// Surgery.
    pub fn operation(&self) -> OperationId {
        self.operation
    }
    pub fn command(&self) -> CommandId {
        self.command
    }
    pub fn worker(&self) -> WorkerInstanceId {
        self.worker
    }
    pub fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }
    /// Area (48 bytes).
    pub fn scope(&self) -> &[u8; limits::SCOPE_LOCK_KEY_LEN] {
        &self.scope
    }
    /// The exact effect of a steady queue; old fixtures don't give it.
    pub fn effect(&self) -> Option<carosign_engine::PlannedEffect> {
        self.effect
    }
}

impl fmt::Debug for ArmedMutationTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ArmedMutationTicket(изменение {}, команда {})",
            self.mutation.as_bytes()[0],
            self.command.as_bytes()[0]
        )
    }
}
