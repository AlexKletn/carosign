use super::BindingError;
use core::fmt;

/// It's not good enough.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SetError {
    /// Empty set - verification is impossible.
    Empty,
    /// More than the limit.
    TooMany {
        actual: usize,
    },
    /// Repeat the registration identifier, including different generations.
    Duplicate,
    /// The bundle failed a form check.
    Binding(BindingError),
}

impl fmt::Display for SetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("пустой набор разрешённых регистраций"),
            Self::TooMany { actual } => write!(f, "в наборе {actual} связок — больше предела"),
            Self::Duplicate => f.write_str("регистрация повторена в наборе"),
            Self::Binding(error) => write!(f, "связка неверна: {error}"),
        }
    }
}

impl core::error::Error for SetError {}

impl From<BindingError> for SetError {
    fn from(value: BindingError) -> Self {
        Self::Binding(value)
    }
}
