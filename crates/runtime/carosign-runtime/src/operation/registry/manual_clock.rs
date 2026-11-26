use crate::ports::TimeSource;
use carosign_core::ids::BootId;
use carosign_core::operation::context::BootInstant;

/// A clock that only goes when it's moved.
///
/// ⚠️ For inspections and for launch without a system, time in combat comes from
/// `carosign-linux`.
#[derive(Debug)]
pub struct ManualClock {
    boot: BootId,
    now_ns: std::sync::Mutex<u64>,
}

impl ManualClock {
    /// The clock is at zero in this load.
    pub fn new(boot: BootId) -> Self {
        Self {
            boot,
            now_ns: std::sync::Mutex::new(0),
        }
    }

    /// Moves the clock forward.
    pub fn advance(&self, by_ns: u64) {
        if let Ok(mut now) = self.now_ns.lock() {
            *now += by_ns;
        }
    }
}

impl TimeSource for ManualClock {
    fn now(&self) -> BootInstant {
        let ns = self.now_ns.lock().map(|value| *value).unwrap_or(0);
        BootInstant::new(self.boot, ns)
    }

    fn boot_id(&self) -> BootId {
        self.boot
    }
}

#[path = "manual_clock.tests.rs"]
#[cfg(test)]
mod tests;
