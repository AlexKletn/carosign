use carosign_core::ids::OperationId;

use super::{EffectOwner, RecordResult, record_slot::RecordSlot};

pub(crate) struct RecordClaim<'a> {
    pub(super) owner: &'a EffectOwner,
    pub(super) id: OperationId,
    pub(super) result: Option<RecordResult>,
}
impl RecordClaim<'_> {
    pub fn result(&self) -> &RecordResult {
        self.result.as_ref().expect("результат удерживается")
    }
    /// Confirms the transfer of the result to the responsible performer.
    pub fn acknowledge(mut self) {
        self.owner
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.id);
        self.result = None;
    }
}
impl Drop for RecordClaim<'_> {
    fn drop(&mut self) {
        if let Some(result) = self.result.take() {
            self.owner
                .records
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(self.id, RecordSlot::Ready(result));
        }
    }
}
