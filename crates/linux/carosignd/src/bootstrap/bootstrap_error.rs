use super::fmt;

/// Recovery failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BootstrapError {
    /// The vault didn't open.
    Store(String),
    /// The scan didn't pass.
    Scan(String),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(message) => write!(f, "хранилище: {message}"),
            Self::Scan(message) => write!(f, "восстановление: {message}"),
        }
    }
}

impl core::error::Error for BootstrapError {}
