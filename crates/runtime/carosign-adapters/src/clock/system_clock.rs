use super::clock_reading::ClockReading;
use carosign_core::ids::BootId;
use carosign_core::operation::context::BootInstant;
use carosign_runtime::TimeSource;

/// Time is taken from the system through []`ClockReading`].
///
/// ⚠️ Your source is mandatory: `carosign-adapters` pulling `libc`So that the checks don't
/// depend on the core. `carosign-linux` (`clock_gettime(CLOCK_BOOTTIME)` and
/// `/proc/sys/kernel/random/boot_id`).
pub struct SystemClock<C: ClockReading> {
    source: C,
}

impl<C: ClockReading> SystemClock<C> {
    /// The clock is over the source.
    pub fn new(source: C) -> Self {
        Self { source }
    }
}

impl<C: ClockReading> TimeSource for SystemClock<C> {
    fn now(&self) -> BootInstant {
        BootInstant::new(self.source.boot_id(), self.source.boottime_ns())
    }

    fn boot_id(&self) -> BootId {
        self.source.boot_id()
    }
}
