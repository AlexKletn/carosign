use super::{JOURNAL_KEY, SwitchFailure, admission, journal, preflight_v3_snapshot};

/// Pending publishes only with a one-time security clearance. There is no real source of
/// security clearance after checking the copy, lock and old binary yet.
#[allow(dead_code)]
pub(super) fn publish_pending(
    db: &redb::Database,
    permit: admission::PendingPermit<'_>,
) -> Result<(), SwitchFailure> {
    publish_pending_inner(db, permit, |write| {
        write.commit().map_err(|_| SwitchFailure::CommitUnknown)
    })
}

pub(super) fn publish_pending_inner(
    db: &redb::Database,
    permit: admission::PendingPermit<'_>,
    commit: impl FnOnce(redb::WriteTransaction) -> Result<(), SwitchFailure>,
) -> Result<(), SwitchFailure> {
    let journal = permit.journal_for(db).ok_or(SwitchFailure::Invalid)?;
    if journal.stage != journal::Stage::Pending {
        return Err(SwitchFailure::Invalid);
    }
    let encoded = journal.encode().map_err(|_| SwitchFailure::Invalid)?;
    let mut write = db.begin_write().map_err(|_| SwitchFailure::Invalid)?;
    write.set_durability(redb::Durability::Immediate);
    // Blocking the writer is held from checking to fixing Pending.
    let read = db.begin_read().map_err(|_| SwitchFailure::Invalid)?;
    preflight_v3_snapshot(read).map_err(|_| SwitchFailure::Invalid)?;
    {
        let mut meta = write
            .open_table(redb::TableDefinition::<&[u8], &[u8]>::new("meta"))
            .map_err(|_| SwitchFailure::Invalid)?;
        if meta
            .insert(JOURNAL_KEY, encoded.as_slice())
            .map_err(|_| SwitchFailure::Invalid)?
            .is_some()
        {
            return Err(SwitchFailure::Invalid);
        }
    }
    commit(write)
}
