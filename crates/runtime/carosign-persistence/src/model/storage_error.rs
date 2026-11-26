use core::fmt;

use super::RecordError;

/// Vault failure.
///
/// Storage failure is separate from domain rejection. `StorageUnavailable` can never be
/// reported as a successful write.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StorageError {
    /// Storage is unavailable (file not opened, record is forbidden, disk crashes).
    Unavailable,
    /// There's something on the disk that doesn't work out.
    Corrupt {
        /// What is it?
        what: &'static str,
    },
    /// The scheme is newer than our understanding.
    UnsupportedSchema {
        /// What we found.
        found: u32,
    },
    /// The record doesn't pass the form check.
    InvalidRecord(RecordError),
    /// It's over the top.
    LimitExceeded {
        /// What a limit.
        limit: &'static str,
        /// How much you got.
        actual: usize,
        allowed: usize,
    },
    /// Generational Conflict (CAS).
    GenerationConflict,
    /// An internal storage error that has nothing to say to the customer.
    Backend,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => f.write_str("хранилище недоступно"),
            Self::Corrupt { what } => write!(f, "повреждённая запись: {what}"),
            Self::UnsupportedSchema { found } => write!(f, "схема {found} не поддерживается"),
            Self::InvalidRecord(e) => write!(f, "запись не проходит проверку: {e}"),
            Self::LimitExceeded {
                limit,
                actual,
                allowed,
            } => {
                write!(f, "предел {limit}: {actual} при допустимом {allowed}")
            }
            Self::GenerationConflict => f.write_str("конфликт поколения"),
            Self::Backend => f.write_str("ошибка хранилища"),
        }
    }
}

impl core::error::Error for StorageError {}

impl From<RecordError> for StorageError {
    fn from(value: RecordError) -> Self {
        Self::InvalidRecord(value)
    }
}
