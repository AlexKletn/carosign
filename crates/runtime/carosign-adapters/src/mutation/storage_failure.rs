/// What came from the vault.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StorageFailure {
    /// The vault is unavailable.
    Unavailable,
    /// Recording unconfirmed.
    NotDurable,
    /// The tape's ruined.
    Corrupt,
}

impl core::fmt::Display for StorageFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "хранилище недоступно",
            Self::NotDurable => "запись не подтверждена",
            Self::Corrupt => "запись испорчена",
        })
    }
}

impl core::error::Error for StorageFailure {}

pub(super) fn map_error(error: carosign_persistence::model::StorageError) -> StorageFailure {
    match error {
        carosign_persistence::model::StorageError::Unavailable => StorageFailure::Unavailable,
        carosign_persistence::model::StorageError::Corrupt { .. }
        | carosign_persistence::model::StorageError::InvalidRecord(_)
        | carosign_persistence::model::StorageError::UnsupportedSchema { .. } => {
            StorageFailure::Corrupt
        }
        carosign_persistence::model::StorageError::LimitExceeded { .. }
        | carosign_persistence::model::StorageError::GenerationConflict
        | carosign_persistence::model::StorageError::Backend => StorageFailure::NotDurable,
    }
}
