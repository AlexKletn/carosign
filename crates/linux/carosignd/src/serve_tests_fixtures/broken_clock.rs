use super::*;

pub(super) struct BrokenClock {
    pub(super) now: BootInstant,
    pub(super) boot: BootId,
}

impl TimeSource for BrokenClock {
    fn now(&self) -> BootInstant {
        self.now
    }
    fn boot_id(&self) -> BootId {
        self.boot
    }
}
