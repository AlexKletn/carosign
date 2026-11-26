use core::fmt;

/// The demands are contradictory.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RequirementConflict;

impl fmt::Debug for RequirementConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RequirementConflict")
    }
}

impl fmt::Display for RequirementConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("требования противоречат цели проверки")
    }
}

impl core::error::Error for RequirementConflict {}
