use carosign_core::ids::AuthenticatorId;

use super::DeviceQueue;

pub(crate) struct PendingDevice<'a> {
    pub(super) queue: &'a DeviceQueue,
    pub(super) id: AuthenticatorId,
    pub(super) ticket: Option<u64>,
}

impl Drop for PendingDevice<'_> {
    fn drop(&mut self) {
        self.queue.release(self.id, self.ticket);
    }
}
