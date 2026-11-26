use core::fmt;

/// What does the fixing with the registry do?
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryAction {
    /// Record the desired state through CAS.
    ApplyDesiredState,
    /// Write down “nothing has changed” and remove the lock.
    RecordNoChange,
}

impl fmt::Debug for RegistryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ApplyDesiredState => "ApplyDesiredState",
            Self::RecordNoChange => "RecordNoChange",
        })
    }
}
