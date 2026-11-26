use super::fmt;

/// The setup doesn't work.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigError {
    /// An unfamiliar field.
    UnknownField(String),
    /// Meaning beyond.
    OutOfBounds {
        field: &'static str,
        /// Count.
        value: u64,
        /// Ceiling.
        max: u64,
    },
    /// Zero or nonnumerical.
    Malformed {
        field: &'static str,
    },
    /// The path is not suitable for the corresponding system call.
    BadPath {
        field: &'static str,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The name of the unknown key is taken from the entire file: the output could
            // reveal the secret or forge a log string by the control byte.
            Self::UnknownField(_) => write!(f, "незнакомое поле"),
            Self::OutOfBounds { field, value, max } => {
                write!(f, "поле {field}: {value} больше потолка {max}")
            }
            Self::Malformed { field } => write!(f, "поле {field} не число"),
            Self::BadPath { field } => write!(f, "поле {field}: недопустимый путь"),
        }
    }
}

impl core::error::Error for ConfigError {}
