use super::{JOURNAL_KEY, PreflightFailure, STAGING_TABLE, TableHandle, journal, staging};

/// Checks staging and source records in one image without changing the base.
/// The result does not confirm lock, backup, authority, or permission to switch; unsupported
/// types of source records cause denial.
#[allow(dead_code)]
pub(crate) fn audit_staging_snapshot(tx: &redb::ReadTransaction) -> Result<(), PreflightFailure> {
    use redb::{ReadableTable, ReadableTableMetadata, TableDefinition};
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind, Value, verify_supported_record};

    const KINDS: [TableKind; 10] = [
        TableKind::Authenticators,
        TableKind::Enrollments,
        TableKind::EnrollmentIndex,
        TableKind::Mutations,
        TableKind::MutationScopeLock,
        TableKind::EffectOutbox,
        TableKind::Idempotency,
        TableKind::ProviderState,
        TableKind::Packages,
        TableKind::TemplateBlobs,
    ];
    let tables = tx.list_tables().map_err(|_| PreflightFailure::Corrupt)?;
    for table in tables {
        if table.name() != "meta"
            && table.name() != STAGING_TABLE
            && !KINDS.iter().any(|kind| kind.name() == table.name())
        {
            return Err(PreflightFailure::UnsupportedRecords);
        }
    }
    if tx
        .list_multimap_tables()
        .map_err(|_| PreflightFailure::Corrupt)?
        .next()
        .is_some()
    {
        return Err(PreflightFailure::UnsupportedRecords);
    }
    let meta = tx
        .open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
        .map_err(|_| PreflightFailure::Corrupt)?;
    if meta.len().map_err(|_| PreflightFailure::Corrupt)? != 2
        || meta
            .get(crate::redb::backend::SCHEMA_KEY)
            .map_err(|_| PreflightFailure::Corrupt)?
            .is_none_or(|v| v.value() != crate::redb::backend::SCHEMA_V3)
    {
        return Err(PreflightFailure::Corrupt);
    }
    let journal_value = meta
        .get(JOURNAL_KEY)
        .map_err(|_| PreflightFailure::Corrupt)?
        .ok_or(PreflightFailure::Corrupt)?;
    let journal =
        journal::Journal::decode(journal_value.value()).map_err(|_| PreflightFailure::Corrupt)?;
    let journal::Stage::Staged(expected) = journal.stage else {
        return Err(PreflightFailure::Corrupt);
    };
    // Before switching, we reconfirm the global uniqueness of identities, not just the shape
    // of individual packet entries in staging.
    crate::installation_records::preflight_v3_packages(tx)
        .map_err(|_| PreflightFailure::Corrupt)?;
    let stage = tx
        .open_table(TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE))
        .map_err(|_| PreflightFailure::Corrupt)?;
    let mut digest = Sha256::new();
    let mut staged_count = 0u64;
    for row in stage.iter().map_err(|_| PreflightFailure::Corrupt)? {
        let (raw_key, raw_value) = row.map_err(|_| PreflightFailure::Corrupt)?;
        let (raw_key, raw_value) = (raw_key.value(), raw_value.value());
        let key = Key::decode(raw_key).map_err(|_| PreflightFailure::Corrupt)?;
        let value = Value::decode(raw_value, key.table).map_err(|_| PreflightFailure::Corrupt)?;
        let source = tx
            .open_table(TableDefinition::<&[u8], &[u8]>::new(key.table.name()))
            .map_err(|_| PreflightFailure::Corrupt)?;
        let source_value = source
            .get(key.source_key)
            .map_err(|_| PreflightFailure::Corrupt)?
            .ok_or(PreflightFailure::Corrupt)?;
        verify_supported_record(&key, &value, &journal.id, source_value.value())
            .map_err(|_| PreflightFailure::UnsupportedRecords)?;
        digest.update(
            u32::try_from(raw_key.len())
                .map_err(|_| PreflightFailure::Corrupt)?
                .to_be_bytes(),
        );
        digest.update(raw_key);
        digest.update(
            u32::try_from(raw_value.len())
                .map_err(|_| PreflightFailure::Corrupt)?
                .to_be_bytes(),
        );
        digest.update(raw_value);
        staged_count = staged_count
            .checked_add(1)
            .ok_or(PreflightFailure::Corrupt)?;
    }
    let mut source_count = 0u64;
    for kind in KINDS {
        let source = tx
            .open_table(TableDefinition::<&[u8], &[u8]>::new(kind.name()))
            .map_err(|_| PreflightFailure::Corrupt)?;
        for row in source.iter().map_err(|_| PreflightFailure::Corrupt)? {
            let (source_key, _) = row.map_err(|_| PreflightFailure::Corrupt)?;
            let key = Key::encode(journal.id, kind, source_key.value())
                .map_err(|_| PreflightFailure::Corrupt)?;
            if stage
                .get(key.as_slice())
                .map_err(|_| PreflightFailure::Corrupt)?
                .is_none()
            {
                return Err(PreflightFailure::Corrupt);
            }
            source_count = source_count
                .checked_add(1)
                .ok_or(PreflightFailure::Corrupt)?;
        }
    }
    let actual: [u8; 32] = digest.finalize().into();
    if staged_count != source_count || staged_count != expected.count || actual != expected.digest {
        return Err(PreflightFailure::Corrupt);
    }
    // Hash and line matching alone don't confirm registry, log, lock, and queue connections,
    // but check them in the same shot, otherwise the individual successful readings may be
    // different versions of the database.
    let source = crate::redb::backend::ReadTxRef { tx };
    let scan = crate::recovery::StartupScan::scan_v3_snapshot(&source)
        .map_err(|_| PreflightFailure::Corrupt)?;
    if !scan.blocked.is_empty() {
        return Err(PreflightFailure::ActiveMutation);
    }
    Ok(())
}
