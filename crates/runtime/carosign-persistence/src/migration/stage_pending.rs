use super::{
    JOURNAL_KEY, MigratableRecord, STAGING_TABLE, SwitchFailure, TableHandle, admission, journal,
    prepare_v3_record, staging,
};

/// Prepares staging and records Staged with a single transaction. The source of access to the
/// real migration is not yet available: the test tolerance does not check the copy, external
/// lock or old binary.
#[allow(dead_code)]
pub(super) fn stage_pending(
    db: &redb::Database,
    permit: admission::StagePermit<'_>,
) -> Result<(), SwitchFailure> {
    stage_pending_inner(db, permit, |write| {
        write.commit().map_err(|_| SwitchFailure::CommitUnknown)
    })
}

pub(super) fn stage_pending_inner(
    db: &redb::Database,
    permit: admission::StagePermit<'_>,
    commit: impl FnOnce(redb::WriteTransaction) -> Result<(), SwitchFailure>,
) -> Result<(), SwitchFailure> {
    use redb::{ReadableTable, ReadableTableMetadata, TableDefinition};
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind as K, Value, verify_supported_record};

    const KINDS: [K; 10] = [
        K::Authenticators,
        K::Enrollments,
        K::EnrollmentIndex,
        K::Mutations,
        K::MutationScopeLock,
        K::EffectOutbox,
        K::Idempotency,
        K::ProviderState,
        K::Packages,
        K::TemplateBlobs,
    ];
    let mut write = db.begin_write().map_err(|_| SwitchFailure::Invalid)?;
    write.set_durability(redb::Durability::Immediate);
    let read = db.begin_read().map_err(|_| SwitchFailure::Invalid)?;
    for table in read.list_tables().map_err(|_| SwitchFailure::Invalid)? {
        if table.name() != "meta"
            && table.name() != STAGING_TABLE
            && !KINDS.iter().any(|kind| kind.name() == table.name())
        {
            return Err(SwitchFailure::Invalid);
        }
    }
    if read
        .list_multimap_tables()
        .map_err(|_| SwitchFailure::Invalid)?
        .next()
        .is_some()
    {
        return Err(SwitchFailure::Invalid);
    }
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let journal = {
        let meta = read
            .open_table(meta_def)
            .map_err(|_| SwitchFailure::Invalid)?;
        if meta.len().map_err(|_| SwitchFailure::Invalid)? != 2
            || meta
                .get(crate::redb::backend::SCHEMA_KEY)
                .map_err(|_| SwitchFailure::Invalid)?
                .is_none_or(|v| v.value() != crate::redb::backend::SCHEMA_V3)
        {
            return Err(SwitchFailure::Invalid);
        }
        let value = meta
            .get(JOURNAL_KEY)
            .map_err(|_| SwitchFailure::Invalid)?
            .ok_or(SwitchFailure::Invalid)?;
        journal::Journal::decode(value.value()).map_err(|_| SwitchFailure::Invalid)?
    };
    if journal.stage != journal::Stage::Pending || !permit.matches(db, &journal.id) {
        return Err(SwitchFailure::Invalid);
    }
    if let Ok(existing) = read.open_table(stage_def)
        && !existing.is_empty().map_err(|_| SwitchFailure::Invalid)?
    {
        return Err(SwitchFailure::Invalid);
    }
    crate::installation_records::preflight_v3_packages(&read)
        .map_err(|_| SwitchFailure::Invalid)?;
    let source = crate::redb::backend::ReadTxRef { tx: &read };
    let scan = crate::recovery::StartupScan::scan_v3_snapshot(&source)
        .map_err(|_| SwitchFailure::Invalid)?;
    if !scan.blocked.is_empty() {
        return Err(SwitchFailure::Invalid);
    }
    let mut hash = Sha256::new();
    let mut count = 0u64;
    {
        let mut target = write
            .open_table(stage_def)
            .map_err(|_| SwitchFailure::Invalid)?;
        for kind in KINDS {
            let source = read
                .open_table(TableDefinition::<&[u8], &[u8]>::new(kind.name()))
                .map_err(|_| SwitchFailure::Invalid)?;
            for row in source.iter().map_err(|_| SwitchFailure::Invalid)? {
                let (key, value) = row.map_err(|_| SwitchFailure::Invalid)?;
                let key = key.value();
                let value = value.value();
                let prepared = match kind {
                    K::Enrollments => prepare_v3_record(MigratableRecord::Enrollment, key, value),
                    K::Mutations => prepare_v3_record(MigratableRecord::Mutation, key, value),
                    K::EffectOutbox => prepare_v3_record(MigratableRecord::Outbox, key, value),
                    K::Idempotency => prepare_v3_record(MigratableRecord::Idempotency, key, value),
                    K::Packages => crate::installation_records::prepare_v3_package(key, value),
                    K::EnrollmentIndex => Ok(value.to_vec()),
                    _ => return Err(SwitchFailure::Invalid),
                }
                .map_err(|_| SwitchFailure::Invalid)?;
                let stage_key =
                    Key::encode(journal.id, kind, key).map_err(|_| SwitchFailure::Invalid)?;
                let stage_value =
                    Value::encode(kind, crate::mutation_plan::sha256(value), &prepared)
                        .map_err(|_| SwitchFailure::Invalid)?;
                verify_supported_record(
                    &Key::decode(&stage_key).map_err(|_| SwitchFailure::Invalid)?,
                    &Value::decode(&stage_value, kind).map_err(|_| SwitchFailure::Invalid)?,
                    &journal.id,
                    value,
                )
                .map_err(|_| SwitchFailure::Invalid)?;
                for bytes in [stage_key.as_slice(), stage_value.as_slice()] {
                    hash.update(
                        u32::try_from(bytes.len())
                            .map_err(|_| SwitchFailure::Invalid)?
                            .to_be_bytes(),
                    );
                    hash.update(bytes);
                }
                count = count.checked_add(1).ok_or(SwitchFailure::Invalid)?;
                if target
                    .insert(stage_key.as_slice(), stage_value.as_slice())
                    .map_err(|_| SwitchFailure::Invalid)?
                    .is_some()
                {
                    return Err(SwitchFailure::Invalid);
                }
            }
        }
    }
    let staged = journal::Journal {
        stage: journal::Stage::Staged(journal::Snapshot {
            count,
            digest: hash.finalize().into(),
        }),
        ..journal
    };
    if !journal.may_advance_to(&staged) {
        return Err(SwitchFailure::Invalid);
    }
    {
        let mut meta = write
            .open_table(meta_def)
            .map_err(|_| SwitchFailure::Invalid)?;
        let encoded = staged.encode().map_err(|_| SwitchFailure::Invalid)?;
        meta.insert(JOURNAL_KEY, encoded.as_slice())
            .map_err(|_| SwitchFailure::Invalid)?;
    }
    drop(read);
    commit(write)
}
