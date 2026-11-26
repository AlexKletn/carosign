use carosign_protocol::validation::WireError;
use core::fmt;

/// The command's not good.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandError {
    /// The wire field doesn't pass the test.
    Wire(WireError),
    /// Unknown listing.
    Enum {
        field: &'static str,
    },
    /// Wrong command.
    Unsupported {
        /// That came.
        what: &'static str,
    },
    /// The right attribute is missing.
    Missing {
        /// What's not.
        field: &'static str,
    },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "провод: {error}"),
            Self::Enum { field } => write!(f, "неизвестное значение {field}"),
            Self::Unsupported { what } => write!(f, "команда {what} не входит в v1"),
            Self::Missing { field } => write!(f, "нет поля {field}"),
        }
    }
}

impl core::error::Error for CommandError {}

impl From<WireError> for CommandError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}
