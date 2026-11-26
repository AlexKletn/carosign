use super::InvalidEpoch;
use core::fmt;

/// Module limit for counting units.
pub const MAX_COUNTER: u64 = u64::MAX;

/// The era of device binding; zero is invalid.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceEpoch(u64);

impl DeviceEpoch {
    /// The first era.
    pub const FIRST: Self = Self(1);

    /// Accepts value, rejects zero.
    pub fn new(value: u64) -> Result<Self, InvalidEpoch> {
        if value == 0 {
            Err(InvalidEpoch)
        } else {
            Ok(Self(value))
        }
    }

    pub fn value(self) -> u64 {
        self.0
    }

    /// The next era; overflow is a mistake, not a twist.
    pub fn next(self) -> Result<Self, InvalidEpoch> {
        self.0.checked_add(1).map(Self).ok_or(InvalidEpoch)
    }
}

impl fmt::Debug for DeviceEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceEpoch({})", self.0)
    }
}

impl TryFrom<u64> for DeviceEpoch {
    type Error = InvalidEpoch;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
