use core::fmt;

/// Catalogue failure.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ManagerError {
    /// The package isn't trusted.
    Untrusted,
    /// It's already installed.
    AlreadyInstalled,
    /// More than the limit.
    TooMany,
}

impl fmt::Debug for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Untrusted => "ManagerError::Untrusted",
            Self::AlreadyInstalled => "ManagerError::AlreadyInstalled",
            Self::TooMany => "ManagerError::TooMany",
        })
    }
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Untrusted => "пакет не доверен",
            Self::AlreadyInstalled => "пакет уже установлен",
            Self::TooMany => "пакетов сверх предела",
        })
    }
}

impl core::error::Error for ManagerError {}
