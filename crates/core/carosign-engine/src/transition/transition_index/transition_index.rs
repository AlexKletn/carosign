use super::TransitionIndexOverflow;
use core::fmt;

/// Transition Index: Starts at 0 and increases by exactly 1.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionIndex(u64);

impl TransitionIndex {
    /// The index before the first transition.
    pub const INITIAL: Self = Self(0);

    pub fn value(self) -> u64 {
        self.0
    }

    /// Takes value (to recover from the journal).
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Next, overflow is a big mistake.
    pub fn next(self) -> Result<Self, TransitionIndexOverflow> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(TransitionIndexOverflow)
    }
}

impl fmt::Debug for TransitionIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransitionIndex({})", self.0)
    }
}
