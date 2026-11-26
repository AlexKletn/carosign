use crate::installation_records::MigratableRecord;
use crate::model::StorageError;
use crate::store::Table;

pub(crate) const SCHEMA_V4: &[u8] = b"BSC1\0\0\0\x04\0\0\0\0";

pub(super) fn corrupt() -> StorageError {
    StorageError::Corrupt {
        what: "неверная запись изменения v4",
    }
}

pub(super) fn kind(table: Table) -> Option<MigratableRecord> {
    match table {
        Table::Enrollments => Some(MigratableRecord::Enrollment),
        Table::Mutations => Some(MigratableRecord::Mutation),
        Table::EffectOutbox => Some(MigratableRecord::Outbox),
        Table::Idempotency => Some(MigratableRecord::Idempotency),
        _ => None,
    }
}
