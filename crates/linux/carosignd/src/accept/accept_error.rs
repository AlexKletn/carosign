use core::fmt;

use carosign_runtime::{authority::SelectionError, ports::AuthorityRefusal};

/// How did the request end?
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AcceptError {
    /// No active registrations.
    NotEnrolled,
    /// The unsub is not matched.
    UnknownSubject,
    /// No authority is given.
    Unauthorized(AuthorityRefusal),
    /// The set didn't come together for another reason.
    Scope(SelectionError),
    /// Identity source is unavailable.
    IdentityUnavailable,
}

impl fmt::Display for AcceptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnrolled => f.write_str("нет активных регистраций"),
            Self::UnknownSubject => f.write_str("субъект не сопоставлен"),
            Self::Unauthorized(reason) => write!(f, "полномочия не даны: {reason:?}"),
            Self::Scope(error) => write!(f, "набор: {error}"),
            Self::IdentityUnavailable => f.write_str("источник личностей недоступен"),
        }
    }
}

impl core::error::Error for AcceptError {}

impl From<SelectionError> for AcceptError {
    fn from(value: SelectionError) -> Self {
        match value {
            SelectionError::NotEnrolled => Self::NotEnrolled,
            other => Self::Scope(other),
        }
    }
}
