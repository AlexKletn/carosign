use super::{CleanupStage, TerminalRecordStage};
use carosign_core::authorization::ValidatedGrant;
use carosign_runtime::OperationActor;
use carosign_runtime::ports::CallerContext;
use std::sync::Mutex;

pub(crate) struct OwnedOperation {
    pub actor: OperationActor,
    pub grant: ValidatedGrant,
    pub(super) deferred_control_step: Option<carosign_runtime::operation::actor::Step>,
    pub(super) owner: CallerContext,
    pub(super) delivery_reserved: bool,
    pub(super) events: std::sync::Arc<Mutex<carosign_runtime::operation::hub::EventHub>>,
    pub(super) terminal_disconnects: Vec<u32>,
    pub lease: Option<carosign_runtime::ports::ResourceLease>,
    pub lease_matches_request: bool,
    pub(super) cleanup_stage: CleanupStage,
    pub(super) terminal_record_stage: TerminalRecordStage,
}

impl OwnedOperation {
    pub(crate) fn belongs_to(&self, caller: &CallerContext) -> bool {
        &self.owner == caller
    }
}

#[cfg(test)]
impl OwnedOperation {
    pub(crate) fn fixture_owner(&self) -> &CallerContext {
        &self.owner
    }
}
