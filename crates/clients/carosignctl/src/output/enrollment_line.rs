use core::fmt;

/// Registration list line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnrollmentLine {
    pub short_id: String,
    /// The mark (as it was given by a man).
    pub label: String,
    /// Waiting for the checks.
    pub pending: bool,
}

impl EnrollmentLine {
    /// Whether it's good for a test.
    ///
    /// ⚠️ Registration waiting for reconciliation is **not* used: it's not "almost working,"
    /// it's "off."
    pub fn usable(&self) -> bool {
        !self.pending
    }
}

impl fmt::Display for EnrollmentLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}  {}", self.short_id, self.label)?;
        if self.pending {
            f.write_str("  (нужна сверка)")?;
        }
        Ok(())
    }
}
