use super::{CleanupOwner, CleanupResult, OperationId, Slot};

pub(super) struct Reservation<'a> {
    pub(super) owner: &'a CleanupOwner,
    pub(super) id: OperationId,
    pub(super) finished: bool,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner
                .slots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(self.id, Slot::Ready(CleanupResult::Interrupted));
        }
    }
}
