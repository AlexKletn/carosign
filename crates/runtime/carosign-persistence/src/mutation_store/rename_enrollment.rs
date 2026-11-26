use carosign_core::{
    enrollment::{EnrollmentLabel, EnrollmentRef},
    identity::SubjectRef,
};

use crate::{
    enrollment_codec::{self, EnrollmentStatus},
    limits::SCOPE_LOCK_KEY_LEN,
    model::StorageError,
    redb::{
        backend::{SCHEMA_KEY, SCHEMA_V3},
        codec::enrollment_index_key,
    },
    store::{MetadataStore, Table},
};

use super::{
    decode_enrollment_v4, encode_enrollment_v4,
    schema::{SCHEMA_V4, corrupt},
};

pub fn rename_enrollment(
    store: &impl MetadataStore,
    subject: SubjectRef,
    target: EnrollmentRef,
    expected_label: &EnrollmentLabel,
    label: &EnrollmentLabel,
    authorize_write: impl FnOnce() -> Result<(), StorageError>,
) -> Result<(), StorageError> {
    let mut tx = store.begin_write()?;
    let schema = tx.get(Table::Meta, SCHEMA_KEY)?;
    let key = target.id.as_bytes();
    let bytes = tx
        .get(Table::Enrollments, key)?
        .ok_or(StorageError::GenerationConflict)?;
    let mut record = match schema.as_deref() {
        Some(SCHEMA_V4) => decode_enrollment_v4(key, &bytes)?,
        None | Some(SCHEMA_V3) => enrollment_codec::decode_v3(&bytes).map_err(|_| corrupt())?,
        _ => return Err(corrupt()),
    };
    if record.subject != subject
        || record.enrollment != target
        || record.status != EnrollmentStatus::Active
    {
        return Err(StorageError::GenerationConflict);
    }
    let index = enrollment_index_key(&record);
    if tx
        .get(Table::MutationScopeLock, &index[..SCOPE_LOCK_KEY_LEN])?
        .is_some()
    {
        return Err(StorageError::GenerationConflict);
    }
    if tx.get(Table::EnrollmentIndex, &index)?.as_deref() != Some(key.as_slice()) {
        return Err(corrupt());
    }
    authorize_write()?;
    if record.display_label() == label.as_str() {
        return Ok(());
    }
    if record.display_label() != expected_label.as_str() {
        return Err(StorageError::GenerationConflict);
    }
    record.display_label = (record.label != label.as_str()).then(|| label.as_str().to_owned());
    let value = if schema.as_deref() == Some(SCHEMA_V4) {
        encode_enrollment_v4(&record)?
    } else {
        enrollment_codec::encode_v3(&record).map_err(|_| corrupt())?
    };
    tx.put(Table::Enrollments, key, &value)?;
    tx.commit_durable()?;
    Ok(())
}

#[cfg(test)]
#[path = "rename_enrollment.tests.rs"]
mod tests;
