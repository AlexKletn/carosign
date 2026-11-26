use core::fmt;

/// Creeds are unavailable or suspicious.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CredentialError {
    /// Not without change: the process is no longer the same as in the core.
    Mismatch,
    /// The connection is already closed.
    Gone,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Mismatch => "креды устарели",
            Self::Gone => "соединение закрыто",
        })
    }
}

impl core::error::Error for CredentialError {}
