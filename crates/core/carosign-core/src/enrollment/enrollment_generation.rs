use super::InvalidGeneration;
use core::fmt;

/// Generation registration; zero is invalid.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnrollmentGeneration(u64);

impl EnrollmentGeneration {
    /// First generation.
    pub const FIRST: Self = Self(1);

    /// Accepts value, rejects zero.
    pub fn new(value: u64) -> Result<Self, InvalidGeneration> {
        if value == 0 {
            Err(InvalidGeneration)
        } else {
            Ok(Self(value))
        }
    }

    pub fn value(self) -> u64 {
        self.0
    }

    /// The next generation; overflow is a big mistake.
    pub fn next(self) -> Result<Self, InvalidGeneration> {
        self.0.checked_add(1).map(Self).ok_or(InvalidGeneration)
    }
}

impl fmt::Debug for EnrollmentGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnrollmentGeneration({})", self.0)
    }
}

impl TryFrom<u64> for EnrollmentGeneration {
    type Error = InvalidGeneration;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
