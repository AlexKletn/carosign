use core::fmt;

/// What the operation does.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    /// Checking on an already named subject.
    Verify,
    /// New registration.
    Enroll,
    /// Deletion of registration.
    Remove,
}

impl fmt::Debug for OperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Verify => "Verify",
            Self::Enroll => "Enroll",
            Self::Remove => "Remove",
        })
    }
}

impl OperationKind {
    /// The view refers to the changing (journalized before sending).
    pub fn is_mutation(self) -> bool {
        matches!(self, Self::Enroll | Self::Remove)
    }
}
