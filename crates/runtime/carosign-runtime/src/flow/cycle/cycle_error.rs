use crate::operation::bindings::SetError;
use crate::ports::AuthorityRefusal;
use core::fmt;

/// What happened.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CycleError {
    /// The source of authority refused.
    Refused(AuthorityRefusal),
    /// Registration set failed.
    Scope(SetError),
    /// The engine rejected the entrance.
    Rejected {
        phase: String,
    },
    /// The operation ended without a result – it should not be.
    NoOutcome,
}

impl fmt::Display for CycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(reason) => write!(f, "полномочия: {reason:?}"),
            Self::Scope(error) => write!(f, "набор регистраций: {error}"),
            Self::Rejected { phase } => write!(f, "движок отверг вход в фазе {phase}"),
            Self::NoOutcome => f.write_str("операция завершилась без итога"),
        }
    }
}

impl core::error::Error for CycleError {}
