use carosign_core::ids::OperationId;

use super::{ControlOwner, WatchResult, slot::Slot};

pub(crate) struct ControlClaim<'a> {
    pub(super) owner: &'a ControlOwner,
    pub(super) operation: OperationId,
    pub(super) result: Option<WatchResult>,
}

impl ControlClaim<'_> {
    /// The operation to which the result belongs.
    pub(crate) fn operation(&self) -> OperationId {
        self.operation
    }

    pub fn result(&self) -> &WatchResult {
        self.result.as_ref().expect("результат удерживается")
    }

    /// Call only after the effects are taken by the responsible performer.
    pub fn acknowledge(mut self) {
        self.owner
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.operation);
        self.result = None;
    }
}

impl Drop for ControlClaim<'_> {
    fn drop(&mut self) {
        if let Some(result) = self.result.take() {
            self.owner
                .slots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(self.operation, Slot::Ready(result));
        }
    }
}
