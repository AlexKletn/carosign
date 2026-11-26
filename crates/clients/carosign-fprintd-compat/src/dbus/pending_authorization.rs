use std::sync::mpsc::{Receiver, TryRecvError};

pub(super) struct PendingAuthorization {
    pub(super) request: dbus::Message,
    pub(super) caller: String,
    pub(super) result: Receiver<Option<u32>>,
}

impl PendingAuthorization {
    pub(super) fn decision(&self) -> Option<Option<u32>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Disconnected) => Some(None),
            Err(TryRecvError::Empty) => None,
        }
    }
}
