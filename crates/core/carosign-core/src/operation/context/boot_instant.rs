use super::BootDeadline;
use crate::ids::BootId;
use core::fmt;

/// A moment of monotonous time along with loading.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BootInstant {
    boot: BootId,
    instant_ns: u64,
}

impl BootInstant {
    /// The moment is in this load.
    pub fn new(boot: BootId, instant_ns: u64) -> Self {
        Self { boot, instant_ns }
    }

    /// Download.
    pub fn boot(self) -> BootId {
        self.boot
    }

    /// Nanseconds of monotonous time.
    pub fn instant_ns(self) -> u64 {
        self.instant_ns
    }

    /// Deadline through `after_ns` from now on.
    pub fn deadline_after(self, after_ns: u64) -> BootDeadline {
        BootDeadline::new(self.boot, self.instant_ns.saturating_add(after_ns))
    }
}

impl fmt::Debug for BootInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BootInstant({} нс)", self.instant_ns)
    }
}
