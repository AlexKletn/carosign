use core::sync::atomic::{AtomicU64, Ordering};

use carosign_core::ids::BootId;
use carosign_core::operation::context::BootInstant;
use carosign_runtime::TimeSource;

/// A watch that a caller moves.
///
/// ⚠️ Only for checks, and for a booth without a kernel, and when you're working, that clock
/// would mean "time goes by when the code decides," and deadlines are not deadlines.
#[derive(Debug)]
pub struct StepClock {
    boot: BootId,
    nanos: AtomicU64,
}

impl StepClock {
    /// Starts from scratch in this download.
    pub fn new(boot: BootId) -> Self {
        Self {
            boot,
            nanos: AtomicU64::new(0),
        }
    }

    /// Moves time forward.
    pub fn advance(&self, by_nanos: u64) {
        let _ = self
            .nanos
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| {
                Some(now.saturating_add(by_nanos))
            });
    }
}

impl TimeSource for StepClock {
    fn now(&self) -> BootInstant {
        BootInstant::new(self.boot, self.nanos.load(Ordering::SeqCst))
    }

    fn boot_id(&self) -> BootId {
        self.boot
    }
}

#[path = "step_clock.tests.rs"]
#[cfg(test)]
mod tests;
