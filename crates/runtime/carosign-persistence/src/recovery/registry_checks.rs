use super::{MutationPhase, MutationRecord, StorageError};

pub(super) fn check_idempotency_for_mutation(
    tx: &dyn crate::store::ReadTransaction,
    record: &MutationRecord,
) -> Result<(), StorageError> {
    use crate::store::Table;
    let mut found = false;
    for (key, bytes) in tx.scan_prefix(Table::Idempotency, &[])? {
        let entry = crate::mutation_records::decode_idempotency(&key, &bytes)?;
        if (entry.operation == record.operation && entry.mutation != Some(record.mutation))
            || (entry.mutation == Some(record.mutation) && entry.operation != record.operation)
        {
            return Err(StorageError::Corrupt {
                what: "идемпотентная запись противоречит журналу изменения",
            });
        }
        found |= entry.operation == record.operation && entry.mutation == Some(record.mutation);
    }
    if record.phase != MutationPhase::Completed && !found {
        return Err(StorageError::Corrupt {
            what: "нет идемпотентной записи незавершённого изменения",
        });
    }
    Ok(())
}

/// Checks both sides of the index in one shot until service is ready.
pub(super) fn check_registry_v3(
    tx: &dyn crate::store::ReadTransaction,
) -> Result<(), StorageError> {
    use crate::store::Table;
    let corrupt = || StorageError::Corrupt {
        what: "реестр и индекс регистраций не согласованы",
    };
    for (key, bytes) in tx.scan_prefix(Table::Enrollments, &[])? {
        let entry = crate::enrollment_codec::decode_v3(&bytes).map_err(|_| corrupt())?;
        if key.as_slice() != entry.enrollment.id.as_bytes()
            || tx
                .get(
                    Table::EnrollmentIndex,
                    &crate::redb::codec::enrollment_index_key(&entry),
                )?
                .as_deref()
                != Some(entry.enrollment.id.as_bytes().as_slice())
        {
            return Err(corrupt());
        }
    }
    for (key, id) in tx.scan_prefix(Table::EnrollmentIndex, &[])? {
        if key.len() != crate::limits::ENROLLMENT_INDEX_KEY_LEN
            || key[crate::limits::SCOPE_LOCK_KEY_LEN..] != id[..]
        {
            return Err(corrupt());
        }
        let bytes = tx.get(Table::Enrollments, &id)?.ok_or_else(corrupt)?;
        let entry = crate::enrollment_codec::decode_v3(&bytes).map_err(|_| corrupt())?;
        if crate::redb::codec::enrollment_index_key(&entry).as_slice() != key.as_slice() {
            return Err(corrupt());
        }
    }
    Ok(())
}
