use core::fmt;

/// The domain has rejected the value or violated the invariant.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    /// All bytes of the identifier are zero or not 16 in length.
    InvalidId,
    /// The unsub's canonical record is incorrect.
    InvalidSubject,
    /// Extract the value beyond the listing.
    UnknownEnumValue(u32),
    /// The recording or format version is not supported.
    UnsupportedVersion {
        /// What's been announced.
        found: u32,
        /// That supports the domain.
        supported: u32,
    },
}

impl fmt::Debug for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidId => "InvalidId",
            Self::InvalidSubject => "InvalidSubject",
            Self::UnknownEnumValue(_) => "UnknownEnumValue(..)",
            Self::UnsupportedVersion { .. } => "UnsupportedVersion { .. }",
        })
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId => f.write_str("идентификатор: нулевой или неверной длины"),
            Self::InvalidSubject => f.write_str("субъект: неверная запись"),
            Self::UnknownEnumValue(v) => write!(f, "значение перечисления {v} не поддерживается"),
            Self::UnsupportedVersion { found, supported } => {
                write!(
                    f,
                    "версия {found} не поддерживается (поддерживается {supported})"
                )
            }
        }
    }
}

impl core::error::Error for DomainError {}
