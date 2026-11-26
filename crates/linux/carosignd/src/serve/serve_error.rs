use super::fmt;

/// How the launch ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ServeError {
    /// No source of identifiers.
    NoIds,
    NoClock,
    /// No authority is given.
    Unauthorized,
    /// The actor didn't accept security clearance.
    Refused,
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoIds => f.write_str("нет источника идентификаторов"),
            Self::NoClock => f.write_str("нет часов"),
            Self::Unauthorized => f.write_str("полномочия не даны"),
            Self::Refused => f.write_str("операция не допущена"),
        }
    }
}

impl core::error::Error for ServeError {}
