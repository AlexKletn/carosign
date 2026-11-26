use core::fmt;

/// The era is invalid: zero or overflow of proven addition.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidEpoch;

impl fmt::Debug for InvalidEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidEpoch")
    }
}

impl fmt::Display for InvalidEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("эпоха: ноль недействителен, переполнение запрещено")
    }
}

impl core::error::Error for InvalidEpoch {}
