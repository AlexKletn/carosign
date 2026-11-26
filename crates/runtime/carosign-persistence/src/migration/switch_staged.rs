use super::{
    JOURNAL_KEY, STAGING_TABLE, SwitchFailure, admission, audit_staging_snapshot, journal, staging,
};

/// The closed atomic switching stage: the production source of the tolerance (lock, verified
/// copy and old binary) does not yet exist, so you can not call it in a normal assembly yet.
/// One write-lock is held from checking the image to commit; the tables before commit change
/// only in it.
#[allow(dead_code)]
pub(super) fn switch_staged(
    db: &redb::Database,
    permit: admission::SwitchPermit<'_>,
) -> Result<(), SwitchFailure> {
    switch_staged_inner(db, permit, |write| {
        write.commit().map_err(|_| SwitchFailure::CommitUnknown)
    })
}

pub(super) fn switch_staged_inner(
    db: &redb::Database,
    permit: admission::SwitchPermit<'_>,
    commit: impl FnOnce(redb::WriteTransaction) -> Result<(), SwitchFailure>,
) -> Result<(), SwitchFailure> {
    use redb::{ReadableTable, TableDefinition};
    use staging::{Key, Value};

    const SCHEMA_V4: &[u8] = b"BSC1\0\0\0\x04\0\0\0\0";
    let mut write = db.begin_write().map_err(|_| SwitchFailure::Invalid)?;
    write.set_durability(redb::Durability::Immediate);
    // As long as a write transaction holds a single redb writer, this snapshot cannot become
    // obsolete because of another commit before we switch.
    let read = db.begin_read().map_err(|_| SwitchFailure::Invalid)?;
    audit_staging_snapshot(&read).map_err(|_| SwitchFailure::Invalid)?;
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let journal = {
        let meta = read
            .open_table(meta_def)
            .map_err(|_| SwitchFailure::Invalid)?;
        let bytes = meta
            .get(JOURNAL_KEY)
            .map_err(|_| SwitchFailure::Invalid)?
            .ok_or(SwitchFailure::Invalid)?;
        journal::Journal::decode(bytes.value()).map_err(|_| SwitchFailure::Invalid)?
    };
    if !permit.matches(db, &journal.id) {
        return Err(SwitchFailure::Invalid);
    }
    let journal::Stage::Staged(snapshot) = journal.stage else {
        return Err(SwitchFailure::Invalid);
    };
    let committed = journal::Journal {
        stage: journal::Stage::Committed(snapshot),
        ..journal
    };
    if !journal.may_advance_to(&committed) {
        return Err(SwitchFailure::Invalid);
    }
    {
        let stage = read
            .open_table(stage_def)
            .map_err(|_| SwitchFailure::Invalid)?;
        for row in stage.iter().map_err(|_| SwitchFailure::Invalid)? {
            let (key, value) = row.map_err(|_| SwitchFailure::Invalid)?;
            let key = Key::decode(key.value()).map_err(|_| SwitchFailure::Invalid)?;
            let value =
                Value::decode(value.value(), key.table).map_err(|_| SwitchFailure::Invalid)?;
            let mut target = write
                .open_table(TableDefinition::<&[u8], &[u8]>::new(key.table.name()))
                .map_err(|_| SwitchFailure::Invalid)?;
            target
                .insert(key.source_key, value.prepared_value)
                .map_err(|_| SwitchFailure::Invalid)?;
        }
    }
    // The audit rejects this table in v3, so it is only now created.
    drop(
        write
            .open_table(TableDefinition::<&[u8], &[u8]>::new(
                "provider_installations",
            ))
            .map_err(|_| SwitchFailure::Invalid)?,
    );
    {
        let mut meta = write
            .open_table(meta_def)
            .map_err(|_| SwitchFailure::Invalid)?;
        meta.insert(crate::redb::backend::SCHEMA_KEY, SCHEMA_V4)
            .map_err(|_| SwitchFailure::Invalid)?;
        let encoded = committed.encode().map_err(|_| SwitchFailure::Invalid)?;
        meta.insert(JOURNAL_KEY, encoded.as_slice())
            .map_err(|_| SwitchFailure::Invalid)?;
    }
    drop(read);
    commit(write)
}

/// Confirms only the form of the completed entry, but not the origin of the base, the actual
/// backup or migration admission.
pub(crate) fn is_committed_v4_journal(key: &[u8], value: &[u8]) -> bool {
    key == JOURNAL_KEY
        && matches!(
            journal::classify(4, Some(value)),
            Ok(journal::RecoveryState::Committed(_))
        )
}
