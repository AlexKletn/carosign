use crate::model::StorageError;
use crate::store::Table;

#[derive(Clone, Copy)]
pub(super) enum ReadSchema {
    Legacy,
    V3,
    V4,
}

pub(super) fn read_schema(
    tx: &dyn crate::store::ReadTransaction,
) -> Result<ReadSchema, StorageError> {
    match tx.get(Table::Meta, crate::redb::backend::SCHEMA_KEY)? {
        None => Ok(ReadSchema::Legacy),
        Some(marker) if marker == crate::redb::backend::SCHEMA_V3 => Ok(ReadSchema::V3),
        Some(marker) if marker == crate::mutation_store::SCHEMA_V4 => Ok(ReadSchema::V4),
        Some(_) => Err(StorageError::Corrupt {
            what: "неизвестный маркер схемы",
        }),
    }
}

// A temporary codec cannot write to a database with a marked or unknown schema.
pub(super) fn require_legacy_schema(
    tx: &dyn crate::store::WriteTransaction,
    legacy_fixture: bool,
) -> Result<(), StorageError> {
    match tx.get(Table::Meta, crate::redb::backend::SCHEMA_KEY)? {
        None if legacy_fixture => Ok(()),
        None => Err(StorageError::Corrupt {
            what: "маркер схемы отсутствует",
        }),
        Some(marker) if marker == crate::redb::backend::SCHEMA_V3 => {
            Err(StorageError::UnsupportedSchema { found: 3 })
        }
        Some(_) => Err(StorageError::Corrupt {
            what: "неизвестный маркер схемы",
        }),
    }
}
