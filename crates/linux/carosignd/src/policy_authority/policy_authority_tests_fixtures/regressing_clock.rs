use super::*;
use carosign_core::operation::context::BootInstant;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct RegressingClock {
    pub(super) boot: BootId,
    pub(super) nanos: AtomicU64,
}

impl TimeSource for RegressingClock {
    fn now(&self) -> BootInstant {
        BootInstant::new(self.boot, self.nanos.load(Ordering::SeqCst))
    }
    fn boot_id(&self) -> BootId {
        self.boot
    }
}
