use crate::installation_records::{MigratableRecord, checked_v4_record_as_v3, prepare_v3_record};
use crate::model::StorageError;
use crate::store::{ReadTransaction, Table};

use super::schema::{SCHEMA_V4, corrupt};

/// Encodes registration for the v4 database with key and structure verification.
pub fn encode_enrollment_v4(
    record: &crate::enrollment_codec::EnrollmentRecord,
) -> Result<Vec<u8>, StorageError> {
    let v3 = crate::enrollment_codec::encode_v3(record).map_err(|_| corrupt())?;
    prepare_v3_record(
        MigratableRecord::Enrollment,
        record.enrollment.id.as_bytes(),
        &v3,
    )
    .map_err(|_| corrupt())
}

/// Verifies the v4 shell and the correspondence of the registration key of the record.
pub fn decode_enrollment_v4(
    key: &[u8],
    bytes: &[u8],
) -> Result<crate::enrollment_codec::EnrollmentRecord, StorageError> {
    let v3 =
        checked_v4_record_as_v3(MigratableRecord::Enrollment, key, bytes).map_err(|_| corrupt())?;
    crate::enrollment_codec::decode_v3(&v3).map_err(|_| corrupt())
}

/// Reads the record according to the version of the database in the same picture as the
/// index.
/// An unknown scheme and an incorrect shell lead to failure.
pub fn decode_enrollment_in_snapshot(
    tx: &dyn ReadTransaction,
    key: &[u8],
    bytes: &[u8],
) -> Result<crate::enrollment_codec::EnrollmentRecord, StorageError> {
    match tx
        .get(Table::Meta, crate::redb::backend::SCHEMA_KEY)?
        .as_deref()
    {
        Some(marker) if marker == SCHEMA_V4 => decode_enrollment_v4(key, bytes),
        Some(marker) if marker != crate::redb::backend::SCHEMA_V3 => Err(corrupt()),
        _ => {
            let record = crate::enrollment_codec::decode_v3(bytes).map_err(|_| corrupt())?;
            if key != record.enrollment.id.as_bytes() {
                return Err(corrupt());
            }
            Ok(record)
        }
    }
}
