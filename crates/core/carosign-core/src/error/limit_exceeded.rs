use core::fmt;

/// The domain limit rejected the meaning.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LimitExceeded {
    /// What a limit.
    pub limit: &'static str,
    /// How much you got.
    pub actual: usize,
    pub allowed: usize,
}

impl fmt::Debug for LimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LimitExceeded({}: {} > {})",
            self.limit, self.actual, self.allowed
        )
    }
}

impl core::error::Error for LimitExceeded {}

impl fmt::Display for LimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "превышен предел {}: {} при допустимом {}",
            self.limit, self.actual, self.allowed
        )
    }
}
