//! Limited analysis of the form of staging under the adopted migration agreement.
//!
//! The format match does not prove that the value corresponds to the original entry, belongs
//! to the current image or is allowed to be transferred.
#![allow(dead_code)]

use crate::limits;

/// Verifies one record with the original value and repeated rigorous transformation.
/// Unsupported views are not considered verified; the mutually unambiguous coverage of the
/// image and the matching of cross-indices is verified by the individual owner.
pub(super) fn verify_supported_record(
    key: &Key<'_>,
    value: &Value<'_>,
    expected_migration: &[u8; 16],
    source_value: &[u8],
) -> Result<(), InvalidStageRecord> {
    use crate::installation_records::{MigratableRecord, prepare_v3_package, prepare_v3_record};
    if &key.migration != expected_migration
        || crate::mutation_plan::sha256(source_value) != value.source_digest
    {
        return Err(InvalidStageRecord);
    }
    if key.table == TableKind::EnrollmentIndex {
        // The index is not StoredValue: its value is the registration ID already entering the
        // end of the key. Full bilateral reconciliation with the registration record is
        // performed StartupScan in the same picture.
        if source_value.len() != 16
            || source_value.iter().all(|byte| *byte == 0)
            || key.source_key[48..] != *source_value
            || key.source_key[..48]
                .chunks_exact(16)
                .any(|part| part.iter().all(|byte| *byte == 0))
            || value.prepared_value != source_value
        {
            return Err(InvalidStageRecord);
        }
        return Ok(());
    }
    let prepared = match key.table {
        TableKind::Enrollments => {
            prepare_v3_record(MigratableRecord::Enrollment, key.source_key, source_value)
        }
        TableKind::Mutations => {
            prepare_v3_record(MigratableRecord::Mutation, key.source_key, source_value)
        }
        TableKind::EffectOutbox => {
            prepare_v3_record(MigratableRecord::Outbox, key.source_key, source_value)
        }
        TableKind::Idempotency => {
            prepare_v3_record(MigratableRecord::Idempotency, key.source_key, source_value)
        }
        TableKind::Packages => prepare_v3_package(key.source_key, source_value),
        _ => return Err(InvalidStageRecord),
    }
    .map_err(|_| InvalidStageRecord)?;
    if prepared != value.prepared_value {
        return Err(InvalidStageRecord);
    }
    Ok(())
}

#[path = "staging.tests.rs"]
#[cfg(test)]
mod tests;

mod invalid_stage_record;
pub(super) use invalid_stage_record::InvalidStageRecord;
mod table_kind;
pub(super) use table_kind::TableKind;
mod key;
pub(super) use key::Key;
mod value;
pub(super) use value::Value;
