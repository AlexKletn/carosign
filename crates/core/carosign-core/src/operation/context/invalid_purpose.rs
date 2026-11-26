use core::fmt;

/// Unknown target.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidPurpose(pub i32);

impl fmt::Debug for InvalidPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InvalidPurpose({})", self.0)
    }
}

impl fmt::Display for InvalidPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "цель проверки {} не поддерживается в v1", self.0)
    }
}

impl core::error::Error for InvalidPurpose {}
