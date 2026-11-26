use super::Phase;
use core::fmt;

/// The message didn't come into its phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PhaseViolation {
    /// Where we are.
    pub phase: Phase,
    /// That came.
    pub what: &'static str,
}

impl fmt::Display for PhaseViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "сообщение {} в фазе {}", self.what, self.phase)
    }
}

impl core::error::Error for PhaseViolation {}
