use super::*;

pub(super) struct AdvancingClock(pub(super) std::sync::atomic::AtomicU64);

impl TimeSource for AdvancingClock {
    fn now(&self) -> BootInstant {
        BootInstant::new(
            Clock.boot_id(),
            self.0.load(std::sync::atomic::Ordering::SeqCst),
        )
    }

    fn boot_id(&self) -> BootId {
        Clock.boot_id()
    }
}
