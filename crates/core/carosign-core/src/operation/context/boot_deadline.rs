use super::BootInstant;
use crate::ids::BootId;
use core::fmt;

/// Deadline tied to the current load.
///
/// ⚠️ It is not stored in the database: at the moment `instant_ns` The only meaning is inside
/// one download.`BootId`](crate::ids::BootId) It's not valid, so let the monotonous time
/// counter match.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BootDeadline {
    boot: BootId,
    instant_ns: u64,
}

impl BootDeadline {
    /// Deadline in this download.
    pub fn new(boot: BootId, instant_ns: u64) -> Self {
        Self { boot, instant_ns }
    }

    /// The download to which it relates.
    pub fn boot(&self) -> BootId {
        self.boot
    }

    /// Monotonous time moment, n's.
    pub fn instant_ns(&self) -> u64 {
        self.instant_ns
    }

    /// Deadline has arrived at the moment `now` same load.
    pub fn is_expired_at(&self, now: BootInstant) -> bool {
        if now.boot() != self.boot {
            // Alien watches: comparison is meaningless, we consider expired.
            return true;
        }
        now.instant_ns() >= self.instant_ns
    }
}

impl fmt::Debug for BootDeadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BootDeadline(at {} нс)", self.instant_ns)
    }
}
