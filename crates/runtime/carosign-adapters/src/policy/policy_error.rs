use core::fmt;

/// Politics doesn't work.
#[derive(Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// Unmarked line.
    Malformed {
        /// Value.
        why: &'static str,
    },
    /// Unfamiliar action.
    UnknownAction(String),
    /// An unfamiliar field.
    UnknownField(String),
    /// Repeat the uid and the subject.
    Duplicate,
    /// Politics is empty and allows nothing; it is not a mistake, but it must be named.
    Empty,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { why } => write!(f, "политика неверна: {why}"),
            // Lines from the policy file may contain secrets and control sequences; they
            // should not be logged.
            Self::UnknownAction(_) => f.write_str("неизвестное действие"),
            Self::UnknownField(_) => f.write_str("незнакомое поле"),
            Self::Duplicate => f.write_str("повтор правила"),
            Self::Empty => f.write_str("политика пуста"),
        }
    }
}

impl fmt::Debug for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { why } => f.debug_struct("Malformed").field("why", why).finish(),
            Self::UnknownAction(_) => f.write_str("UnknownAction(<скрыто>)"),
            Self::UnknownField(_) => f.write_str("UnknownField(<скрыто>)"),
            Self::Duplicate => f.write_str("Duplicate"),
            Self::Empty => f.write_str("Empty"),
        }
    }
}

impl core::error::Error for PolicyError {}
