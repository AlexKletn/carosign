use super::{StorageError, fmt};

/// Recording's not good.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RecordError {
    /// The shell doesn't work.
    Envelope(StorageError),
    /// The scheme is not the same: clear migration is needed.
    UnsupportedSchema {
        /// What we found.
        found: u32,
    },
    /// There's no registry record in meaning.
    NotAnEnrollment,
    /// The field is not the same length or empty.
    BadField {
        field: &'static str,
    },
    UnknownStatus {
        value: i32,
    },
    ZeroGeneration,
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => write!(f, "оболочка: {error}"),
            Self::UnsupportedSchema { found } => {
                write!(f, "схема {found}: нужна явная миграция")
            }
            Self::NotAnEnrollment => f.write_str("значение не содержит записи реестра"),
            Self::BadField { field } => write!(f, "поле {field} пустое или неверной длины"),
            Self::UnknownStatus { value } => write!(f, "неизвестный статус {value}"),
            Self::ZeroGeneration => f.write_str("поколение ноль"),
        }
    }
}

impl core::error::Error for RecordError {}

impl From<StorageError> for RecordError {
    fn from(value: StorageError) -> Self {
        Self::Envelope(value)
    }
}
