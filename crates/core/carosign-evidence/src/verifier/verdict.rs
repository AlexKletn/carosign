use crate::assessment::Assessment;
use crate::material::EvidenceError;
use core::fmt;

/// The result of the proof check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// Accepted with grade.
    Accepted(Assessment),
    /// Rejected for a reason.
    Rejected(EvidenceError),
}

impl Verdict {
    /// Copy that.
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted(assessment) => write!(f, "принято: {assessment}"),
            Self::Rejected(error) => write!(f, "отвергнуто: {error}"),
        }
    }
}
