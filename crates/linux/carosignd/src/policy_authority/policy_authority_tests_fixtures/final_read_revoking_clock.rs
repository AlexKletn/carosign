use super::*;
use carosign_core::operation::context::BootInstant;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct RevokingClock {
    pub(super) boot: BootId,
    pub(super) path: PathBuf,
    pub(super) reads: AtomicUsize,
}

impl TimeSource for RevokingClock {
    fn now(&self) -> BootInstant {
        // The first check belongs to the preliminary decision, the second - the publication
        // of permission by the owner of the picture.
        if self.reads.fetch_add(1, Ordering::SeqCst) == 1 {
            std::fs::write(&self.path, "# revoked\n").unwrap();
        }
        BootInstant::new(self.boot, 0)
    }
    fn boot_id(&self) -> BootId {
        self.boot
    }
}
