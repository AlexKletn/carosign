use crate::operation::bindings::SetError;
use core::fmt;

/// Why didn't the set come together?
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectionError {
    /// There is no active registration, there is nothing to check.
    ///
    /// A subject can legitimately have no enrollments. This outcome does not authorize a
    /// wildcard scan.
    NotEnrolled,
    /// Active recording of someone else's authenticator: this is a corruption of the
    /// registry, not a reason to expand the set.
    WrongAuthenticator,
    /// The record is waiting for reconciliation: you can’t check it (document 10).
    PendingReconciliation,
    /// The kit didn't pass the shape check.
    Set(SetError),
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnrolled => f.write_str("нет активных регистраций для проверки"),
            Self::WrongAuthenticator => f.write_str("запись принадлежит другому аутентификатору"),
            Self::PendingReconciliation => f.write_str("регистрация ждёт сверки"),
            Self::Set(error) => write!(f, "набор неверен: {error}"),
        }
    }
}

impl core::error::Error for SelectionError {}

impl From<SetError> for SelectionError {
    fn from(value: SetError) -> Self {
        Self::Set(value)
    }
}
