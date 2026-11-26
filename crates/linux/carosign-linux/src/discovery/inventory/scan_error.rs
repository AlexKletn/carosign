use core::fmt;

/// I can't collect any inventory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScanError {
    /// The catalogue is unreadable.
    Unreadable {
        /// What.
        path: String,
    },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path } => write!(f, "не читается {path}"),
        }
    }
}

impl core::error::Error for ScanError {}
