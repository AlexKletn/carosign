use core::fmt;

/// The type of refusal when changing.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationFailure {
    /// Recording is another generation - rejection before sending.
    GenerationConflict,
    /// The provider doesn't know how to do that.
    Unsupported,
    /// There's no room in the registrations.
    LimitReached,
    /// Secure storage failed.
    SecureStoreRejected,
    Revoked,
}

impl fmt::Debug for MutationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GenerationConflict => "GenerationConflict",
            Self::Unsupported => "Unsupported",
            Self::LimitReached => "LimitReached",
            Self::SecureStoreRejected => "SecureStoreRejected",
            Self::Revoked => "Revoked",
        })
    }
}
