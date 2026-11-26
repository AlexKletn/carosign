use crate::enrollment_codec;
use crate::limits;
use crate::model::{MutationRecord, StorageError};
use crate::store::Table;

/// Verifies the full ledger snapshot in the same recording transaction that creates a lock or
/// permission to send.
pub(super) fn check_current_scope(
    tx: &dyn crate::store::WriteTransaction,
    mutation: &MutationRecord,
) -> Result<(), StorageError> {
    check_current_scope_with(tx, mutation, enrollment_codec::decode).map(|_| ())
}

/// Same check with the Registry Registry Registry Codec v3.
pub(crate) fn check_current_scope_v3(
    tx: &dyn crate::store::WriteTransaction,
    mutation: &MutationRecord,
) -> Result<(), StorageError> {
    read_current_scope_v3(tx, mutation).map(|_| ())
}

pub(crate) fn read_current_scope_v3(
    tx: &dyn crate::store::WriteTransaction,
    mutation: &MutationRecord,
) -> Result<Vec<crate::projection::RegistryEntry>, StorageError> {
    check_current_scope_with(tx, mutation, enrollment_codec::decode_v3)
}

fn check_current_scope_with(
    tx: &dyn crate::store::WriteTransaction,
    mutation: &MutationRecord,
    decode: fn(
        &[u8],
    ) -> Result<
        crate::enrollment_codec::EnrollmentRecord,
        crate::enrollment_codec::RecordError,
    >,
) -> Result<Vec<crate::projection::RegistryEntry>, StorageError> {
    let plan = mutation.plan.as_ref().ok_or(StorageError::Corrupt {
        what: "нет плана изменения",
    })?;
    let entries = read_indexed_scope(tx, mutation, decode)?;
    crate::projection::check_scope(plan, &entries).map_err(|_| StorageError::GenerationConflict)?;
    if let Some(new_id) = plan.new_enrollment
        && tx.get(Table::Enrollments, new_id.as_bytes())?.is_some()
    {
        return Err(StorageError::GenerationConflict);
    }
    Ok(entries)
}

pub(crate) fn read_indexed_scope_v3(
    tx: &dyn crate::store::WriteTransaction,
    mutation: &MutationRecord,
) -> Result<Vec<crate::projection::RegistryEntry>, StorageError> {
    read_indexed_scope(tx, mutation, enrollment_codec::decode_v3)
}

fn read_indexed_scope(
    tx: &dyn crate::store::WriteTransaction,
    mutation: &MutationRecord,
    decode: fn(&[u8]) -> Result<enrollment_codec::EnrollmentRecord, enrollment_codec::RecordError>,
) -> Result<Vec<crate::projection::RegistryEntry>, StorageError> {
    let mut entries = Vec::new();
    let subject_key = mutation.subject.to_canonical();
    let scope_key = mutation.scope_key();
    for (key, value) in tx.scan_prefix(Table::EnrollmentIndex, &scope_key)? {
        if key.len() != limits::ENROLLMENT_INDEX_KEY_LEN
            || key[limits::SCOPE_LOCK_KEY_LEN..] != value[..]
        {
            return Err(StorageError::Corrupt {
                what: "неверный индекс регистрации",
            });
        }
        let bytes = tx
            .get(Table::Enrollments, &value)?
            .ok_or(StorageError::Corrupt {
                what: "индекс без регистрации",
            })?;
        let record = decode(&bytes).map_err(|_| StorageError::Corrupt {
            what: "неверная регистрация в области",
        })?;
        if crate::redb::codec::enrollment_index_key(&record).as_slice() != key.as_slice() {
            return Err(StorageError::Corrupt {
                what: "индекс не соответствует регистрации",
            });
        }
        entries.push(crate::projection::RegistryEntry::from_record(&record));
    }
    // The index itself does not prove the completeness of the area: a lost row of the index
    // would hide the existing registration from the plan and CAS verification.
    for (key, bytes) in tx.scan_prefix(Table::Enrollments, &[])? {
        let record = decode(&bytes).map_err(|_| StorageError::Corrupt {
            what: "неверная запись реестра",
        })?;
        if key.as_slice() != record.enrollment.id.as_bytes() {
            return Err(StorageError::Corrupt {
                what: "ключ регистрации не соответствует записи",
            });
        }
        if record.subject.to_canonical() == subject_key
            && record.enrollment.authenticator == mutation.authenticator
            && tx
                .get(
                    Table::EnrollmentIndex,
                    &crate::redb::codec::enrollment_index_key(&record),
                )?
                .as_deref()
                != Some(record.enrollment.id.as_bytes().as_slice())
        {
            return Err(StorageError::Corrupt {
                what: "регистрация области отсутствует в индексе",
            });
        }
    }
    Ok(entries)
}
