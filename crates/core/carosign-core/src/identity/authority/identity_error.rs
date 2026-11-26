use core::fmt;

/// Identity source port's down.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// The subject is not identified or matched unequivocally.
    UnknownSubject,
    /// The caller is forbidden to work with this subject.
    NotPermitted,
    /// The source of the identities is unavailable - rejection, not admission.
    Unavailable,
}

impl fmt::Debug for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownSubject => "UnknownSubject",
            Self::NotPermitted => "NotPermitted",
            Self::Unavailable => "Unavailable",
        })
    }
}
