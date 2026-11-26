use core::fmt;

/// What should be the fixed state of registration.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnrollmentStatus {
    /// It works.
    Active,
    /// Waiting for a check; you can't check it.
    PendingReconciliation,
    /// Unavailable (provider removed, etc.).
    Unavailable,
}

impl fmt::Debug for EnrollmentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Active => "Active",
            Self::PendingReconciliation => "PendingReconciliation",
            Self::Unavailable => "Unavailable",
        })
    }
}
