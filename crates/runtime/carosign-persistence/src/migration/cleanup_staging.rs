use super::{
    CleanupFailure, JOURNAL_KEY, PreflightFailure, STAGING_TABLE, TableHandle, journal, staging,
};

/// Removes only the verified staging of the completed migration. The log, backup and main
/// tables are saved. `CommitUnknown` It requires reading a new image; it's not a permit to
/// start a migration.
#[allow(dead_code)]
pub(crate) fn cleanup_committed_staging(db: &redb::Database) -> Result<bool, CleanupFailure> {
    cleanup_committed_staging_inner(db)
}

fn cleanup_committed_staging_inner(db: &redb::Database) -> Result<bool, CleanupFailure> {
    use redb::{ReadableTable, ReadableTableMetadata, TableDefinition};
    use sha2::{Digest, Sha256};
    use staging::{Key, Value};

    const V4: &[u8] = b"BSC1\0\0\0\x04\0\0\0\0";
    let mut tx = db.begin_write().map_err(|_| CleanupFailure::Invalid)?;
    tx.set_durability(redb::Durability::Immediate);
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let existing = tx
        .list_tables()
        .map_err(|_| PreflightFailure::Corrupt)?
        .map(|table| table.name().to_string())
        .collect::<Vec<_>>();
    const KNOWN_TABLES: &[&str] = &[
        "meta",
        STAGING_TABLE,
        "authenticators",
        "enrollments",
        "enrollment_index",
        "mutations",
        "mutation_scope_lock",
        "effect_outbox",
        "idempotency",
        "provider_state",
        "packages",
        "template_blobs",
        "provider_installations",
    ];
    if existing
        .iter()
        .any(|name| !KNOWN_TABLES.contains(&name.as_str()))
        || !existing.iter().any(|name| name == "meta")
        || tx
            .list_multimap_tables()
            .map_err(|_| PreflightFailure::Corrupt)?
            .next()
            .is_some()
    {
        return Err(CleanupFailure::Invalid);
    }
    let journal = {
        let meta = tx
            .open_table(meta_def)
            .map_err(|_| PreflightFailure::Corrupt)?;
        if meta.len().map_err(|_| PreflightFailure::Corrupt)? != 2
            || meta
                .get(crate::redb::backend::SCHEMA_KEY)
                .map_err(|_| PreflightFailure::Corrupt)?
                .is_none_or(|value| value.value() != V4)
        {
            return Err(CleanupFailure::Invalid);
        }
        let bytes = meta
            .get(JOURNAL_KEY)
            .map_err(|_| PreflightFailure::Corrupt)?
            .ok_or(PreflightFailure::Corrupt)?;
        journal::Journal::decode(bytes.value()).map_err(|_| PreflightFailure::Corrupt)?
    };
    let journal::Stage::Committed(expected) = journal.stage else {
        return Err(CleanupFailure::Invalid);
    };
    if !existing.iter().any(|name| name == STAGING_TABLE) {
        return Ok(false);
    }
    {
        let stage = tx
            .open_table(stage_def)
            .map_err(|_| PreflightFailure::Corrupt)?;
        let mut digest = Sha256::new();
        let mut count = 0u64;
        for row in stage.iter().map_err(|_| PreflightFailure::Corrupt)? {
            let (key, value) = row.map_err(|_| PreflightFailure::Corrupt)?;
            let parsed = Key::decode(key.value()).map_err(|_| PreflightFailure::Corrupt)?;
            if parsed.migration != journal.id {
                return Err(CleanupFailure::Invalid);
            }
            Value::decode(value.value(), parsed.table).map_err(|_| PreflightFailure::Corrupt)?;
            for bytes in [key.value(), value.value()] {
                digest.update(
                    u32::try_from(bytes.len())
                        .map_err(|_| PreflightFailure::Corrupt)?
                        .to_be_bytes(),
                );
                digest.update(bytes);
            }
            count = count.checked_add(1).ok_or(PreflightFailure::Corrupt)?;
        }
        let actual: [u8; 32] = digest.finalize().into();
        if count != expected.count || actual != expected.digest {
            return Err(CleanupFailure::Invalid);
        }
    }
    if !tx
        .delete_table(stage_def)
        .map_err(|_| PreflightFailure::Corrupt)?
    {
        return Err(CleanupFailure::Invalid);
    }
    tx.commit().map_err(|_| CleanupFailure::CommitUnknown)?;
    Ok(true)
}
