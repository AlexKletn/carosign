use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use crate::control_watch::RevocationSender;

pub(crate) struct CancellationTarget {
    pub(super) sender: RevocationSender,
    pub(super) rollback: Option<Arc<AtomicBool>>,
    pub(super) handling: Arc<AtomicBool>,
    pub(super) stopped: Arc<AtomicBool>,
    pub(super) requested: Mutex<bool>,
}

impl CancellationTarget {
    pub(crate) fn request(&self) -> Result<(), ()> {
        let mut requested = self.requested.lock().map_err(|_| ())?;
        if self.stopped.load(Ordering::Acquire) {
            return Err(());
        }
        if *requested {
            return Ok(());
        }
        if let Some(rollback) = &self.rollback {
            rollback.store(true, Ordering::Release);
            self.handling.store(true, Ordering::Release);
        } else {
            self.handling.store(true, Ordering::Release);
            self.sender
                .cancel_and_wait(Duration::from_secs(2))
                .map_err(|_| ())?;
        }
        *requested = true;
        Ok(())
    }
}
