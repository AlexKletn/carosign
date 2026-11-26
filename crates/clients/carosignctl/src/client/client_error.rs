use core::fmt;

/// Customer rejection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClientError {
    /// Service's not answering.
    Unavailable,
    /// The service refused.
    Refused {
        /// I told you.
        message: String,
    },
    /// The answer doesn't fit the command.
    Unexpected,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("служба недоступна"),
            Self::Refused { message } => write!(f, "служба отказала: {message}"),
            Self::Unexpected => f.write_str("ответ не подходит к команде"),
        }
    }
}

impl core::error::Error for ClientError {}
