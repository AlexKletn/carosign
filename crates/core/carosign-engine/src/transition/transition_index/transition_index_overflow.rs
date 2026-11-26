use core::fmt;

/// The index has crossed the limit `u64`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TransitionIndexOverflow;

impl fmt::Debug for TransitionIndexOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TransitionIndexOverflow")
    }
}

impl fmt::Display for TransitionIndexOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("счётчик переходов переполнен")
    }
}

impl core::error::Error for TransitionIndexOverflow {}
