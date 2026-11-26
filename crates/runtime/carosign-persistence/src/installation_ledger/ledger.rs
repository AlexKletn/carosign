use super::*;

/// Isolated experimental base: do not change v3 user metadata.
pub(crate) struct Ledger {
    db: Database,
}

impl Ledger {
    pub(crate) fn create(path: &Path) -> Result<Self, LedgerError> {
        use std::os::unix::fs::OpenOptionsExt;
        if !path.is_absolute() {
            return Err(LedgerError::Invalid);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| LedgerError::Unavailable)?;
        let db = Database::builder()
            .create_file(file)
            .map_err(|_| LedgerError::Unavailable)?;
        let tx = db.begin_write().map_err(|_| LedgerError::Unavailable)?;
        {
            tx.open_table(RECORDS)
                .map_err(|_| LedgerError::Unavailable)?;
        }
        tx.commit().map_err(|_| LedgerError::Unavailable)?;
        std::fs::File::open(path.parent().ok_or(LedgerError::Invalid)?)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| LedgerError::Unavailable)?;
        Ok(Self { db })
    }

    pub(crate) fn open(path: &Path) -> Result<Self, LedgerError> {
        let db = Database::open(path).map_err(|_| LedgerError::Unavailable)?;
        let tx = db.begin_read().map_err(|_| LedgerError::Unavailable)?;
        {
            tx.open_table(RECORDS).map_err(|_| LedgerError::Corrupt)?;
        }
        drop(tx);
        Ok(Self { db })
    }

    /// Audit comparison and recording are performed by a single transaction.
    /// The caller is obliged to provide system authority and package verification.
    pub(crate) fn compare_and_set(
        &self,
        record: &InstallationRecord,
        expected: Option<u64>,
    ) -> Result<(), LedgerError> {
        let encoded = encode(record)?;
        let k = key(record);
        let tx = self
            .db
            .begin_write()
            .map_err(|_| LedgerError::Unavailable)?;
        {
            let mut table = tx
                .open_table(RECORDS)
                .map_err(|_| LedgerError::Unavailable)?;
            let old = table
                .get(k.as_slice())
                .map_err(|_| LedgerError::Unavailable)?
                .map(|v| decode(&k, v.value()))
                .transpose()?;
            match (old, expected) {
                (None, None) if record.revision == 1 && !record.revoked => {}
                (Some(old), Some(rev))
                    if old.revision == rev
                        && rev.checked_add(1) == Some(record.revision)
                        && !old.revoked
                        && old.fingerprint == record.fingerprint => {}
                _ => return Err(LedgerError::Conflict),
            }
            table
                .insert(k.as_slice(), encoded.as_slice())
                .map_err(|_| LedgerError::Unavailable)?;
        }
        tx.commit().map_err(|_| LedgerError::Unavailable)
    }

    pub(crate) fn read(&self, k: &[u8; 48]) -> Result<Option<InstallationRecord>, LedgerError> {
        let tx = self.db.begin_read().map_err(|_| LedgerError::Unavailable)?;
        let table = tx
            .open_table(RECORDS)
            .map_err(|_| LedgerError::Unavailable)?;
        table
            .get(k.as_slice())
            .map_err(|_| LedgerError::Unavailable)?
            .map(|v| decode(k, v.value()))
            .transpose()
    }
}
