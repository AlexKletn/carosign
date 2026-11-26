use core::fmt;

/// The command doesn't understand.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandError {
    /// Unknown command.
    Unknown {
        /// Introduced.
        word: String,
    },
    /// There's no argument.
    Missing {
        what: &'static str,
    },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { word } => write!(f, "нет такой команды: {word}"),
            Self::Missing { what } => write!(f, "не хватает: {what}"),
        }
    }
}

impl core::error::Error for CommandError {}
