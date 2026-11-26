use core::fmt;

/// Why didn't you pack?
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BootError {
    /// There's not enough ports.
    MissingPorts {
        /// What.
        missing: Vec<&'static str>,
    },
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPorts { missing } => {
                write!(f, "не заданы порты: {}", missing.join(", "))
            }
        }
    }
}

impl core::error::Error for BootError {}
