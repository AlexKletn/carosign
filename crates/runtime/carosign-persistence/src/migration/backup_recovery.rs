use super::{
    JOURNAL_KEY, SwitchFailure, audit_staging_snapshot, journal, preflight_v3_snapshot_inner,
};

/// Re-checks the retained copy with respect to the incomplete migration log. This is just
/// reading: administrative selection, external fencing, origin of the copy and actual
/// recovery are not confirmed here.
#[allow(dead_code)]
pub(super) fn inspect_recovery_backup(
    db: &redb::Database,
    backup: &crate::redb::backend::VerifiedV3Backup,
    max_backup_size: u64,
) -> Result<journal::Stage, SwitchFailure> {
    use redb::{ReadableTable, TableDefinition};
    let identity = backup
        .recheck(max_backup_size)
        .map_err(|_| SwitchFailure::Invalid)?;
    let read = db.begin_read().map_err(|_| SwitchFailure::Invalid)?;
    let meta = read
        .open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
        .map_err(|_| SwitchFailure::Invalid)?;
    for row in meta.iter().map_err(|_| SwitchFailure::Invalid)? {
        let (key, _) = row.map_err(|_| SwitchFailure::Invalid)?;
        if key.value() != crate::redb::backend::SCHEMA_KEY && key.value() != JOURNAL_KEY {
            return Err(SwitchFailure::Invalid);
        }
    }
    let schema = meta
        .get(crate::redb::backend::SCHEMA_KEY)
        .map_err(|_| SwitchFailure::Invalid)?;
    if schema.is_none_or(|value| value.value() != crate::redb::backend::SCHEMA_V3) {
        return Err(SwitchFailure::Invalid);
    }
    let bytes = meta
        .get(JOURNAL_KEY)
        .map_err(|_| SwitchFailure::Invalid)?
        .ok_or(SwitchFailure::Invalid)?;
    let journal = journal::Journal::decode(bytes.value()).map_err(|_| SwitchFailure::Invalid)?;
    if !matches!(
        journal.stage,
        journal::Stage::Pending | journal::Stage::Staged(_)
    ) || journal.backup_device != identity.device
        || journal.backup_inode != identity.inode
        || journal.backup_length != identity.length
        || journal.backup_digest != identity.digest
    {
        return Err(SwitchFailure::Invalid);
    }
    match journal.stage {
        journal::Stage::Pending => {
            preflight_v3_snapshot_inner(&read, true).map_err(|_| SwitchFailure::Invalid)?;
        }
        journal::Stage::Staged(_) => {
            audit_staging_snapshot(&read).map_err(|_| SwitchFailure::Invalid)?;
        }
        _ => return Err(SwitchFailure::Invalid),
    }
    if backup
        .recheck(max_backup_size)
        .map_err(|_| SwitchFailure::Invalid)?
        != identity
    {
        return Err(SwitchFailure::Invalid);
    }
    Ok(journal.stage)
}
