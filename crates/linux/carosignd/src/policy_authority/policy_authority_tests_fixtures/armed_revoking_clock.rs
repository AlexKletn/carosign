use super::*;
use carosign_core::operation::context::BootInstant;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct RevokingClock {
    pub(super) boot: BootId,
    pub(super) path: PathBuf,
    pub(super) armed: AtomicBool,
}

impl TimeSource for RevokingClock {
    fn now(&self) -> BootInstant {
        if self.armed.swap(false, Ordering::SeqCst) {
            std::fs::write(&self.path, "# revoked\n").unwrap();
        }
        BootInstant::new(self.boot, 0)
    }
    fn boot_id(&self) -> BootId {
        self.boot
    }
}
