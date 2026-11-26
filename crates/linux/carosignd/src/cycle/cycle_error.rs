use super::fmt;

/// It's not good for a shot.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CycleError {
    /// The frame doesn't understand.
    Malformed,
    /// The request is not about the operation - the service does not yet know how to do this.
    NotAnOperation,
}

impl fmt::Display for CycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => "кадр не разбирается",
            Self::NotAnOperation => "просьба не про операцию",
        })
    }
}

impl core::error::Error for CycleError {}
