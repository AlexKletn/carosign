use core::fmt;

/// Request class.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Verification for the system consumer.
    SystemAuth,
    /// Managing the human push factor.
    InteractiveManage,
    /// Diagnosis.
    Diagnostic,
}

impl fmt::Debug for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SystemAuth => "SystemAuth",
            Self::InteractiveManage => "InteractiveManage",
            Self::Diagnostic => "Diagnostic",
        })
    }
}
