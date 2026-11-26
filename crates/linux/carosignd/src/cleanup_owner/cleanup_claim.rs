use super::{CleanupOwner, CleanupResult, OperationId, Slot};

pub(crate) struct CleanupClaim<'a> {
    pub(super) owner: &'a CleanupOwner,
    pub(super) id: OperationId,
    pub(super) result: Option<CleanupResult>,
}

#[allow(dead_code)]
impl CleanupClaim<'_> {
    /// The operation that owns the retained result.
    pub fn operation(&self) -> OperationId {
        self.id
    }
    pub fn result(&self) -> &CleanupResult {
        self.result.as_ref().expect("результат удерживается")
    }
    /// Confirms the transfer to the responsible contractor.
    pub fn acknowledge(mut self) {
        self.owner
            .slots
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.id);
        self.result = None;
    }
}

impl Drop for CleanupClaim<'_> {
    fn drop(&mut self) {
        if let Some(result) = self.result.take() {
            self.owner
                .slots
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(self.id, Slot::Ready(result));
        }
    }
}
