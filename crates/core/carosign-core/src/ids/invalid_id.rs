use crate::error::DomainError;
use core::fmt;

/// Zero length identifier is always rejected.
///
/// All 16 bytes are zero, meaning "not assigned" rather than "no assigned."
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidId;

impl fmt::Debug for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidId")
    }
}

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("идентификатор: все байты нулевые или длина не 16")
    }
}

impl core::error::Error for InvalidId {}

impl From<InvalidId> for DomainError {
    fn from(_: InvalidId) -> Self {
        DomainError::InvalidId
    }
}
