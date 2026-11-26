use super::{
    INSTALLATIONS, InvalidInstallation, META, PACKAGES, SCHEMA_KEY, SnapshotRefusal, StoredValue,
    Value, decode_package, envelope_with, sha256, unenvelope_with,
};
use prost::Message;
use redb::ReadableTable;
use std::collections::BTreeSet;

/// Prepares the v3 packet for explicit migration without changing the base or creating the
/// installation record.
#[allow(dead_code)] // Called only by the future owner of the migration log.
pub(crate) fn prepare_v3_package(key: &[u8], bytes: &[u8]) -> Result<Vec<u8>, InvalidInstallation> {
    let payload = unenvelope_with(bytes, sha256).map_err(|_| InvalidInstallation)?;
    let mut value = StoredValue::decode(payload.as_slice()).map_err(|_| InvalidInstallation)?;
    if value.schema_version != 3
        || value.encode_to_vec() != payload
        || !matches!(value.value, Some(Value::PackageRecord(_)))
    {
        return Err(InvalidInstallation);
    }
    value.schema_version = 4;
    let output = envelope_with(&value.encode_to_vec(), sha256).map_err(|_| InvalidInstallation)?;
    // The same key, content, and limit check that is used when reading v4; the spoilage does
    // not turn into an empty or new setup.
    decode_package(key, &output)?;
    Ok(output)
}

/// A type of record for which there is already a v3 checker.
/// It does not include indexes, locks and views without a checking decoder.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum MigratableRecord {
    Enrollment,
    Mutation,
    Outbox,
    Idempotency,
}

/// Prepares a separate verified record without changing the picture or allowing migration.
/// The consistency of the tables, the lack of active changes, the backup and the log remain
/// mandatory checks of the migration owner.
#[allow(dead_code)]
pub(crate) fn prepare_v3_record(
    kind: MigratableRecord,
    key: &[u8],
    bytes: &[u8],
) -> Result<Vec<u8>, InvalidInstallation> {
    let payload = unenvelope_with(bytes, sha256).map_err(|_| InvalidInstallation)?;
    let mut stored = StoredValue::decode(payload.as_slice()).map_err(|_| InvalidInstallation)?;
    if stored.schema_version != 3 || stored.encode_to_vec() != payload {
        return Err(InvalidInstallation);
    }
    match (kind, &stored.value) {
        (MigratableRecord::Enrollment, Some(Value::Enrollment(_))) => {
            let record =
                crate::enrollment_codec::decode_v3(bytes).map_err(|_| InvalidInstallation)?;
            if key != record.enrollment.id.as_bytes() {
                return Err(InvalidInstallation);
            }
        }
        (MigratableRecord::Mutation, Some(Value::Mutation(_))) => {
            let record =
                crate::mutation_records::decode_mutation(bytes).map_err(|_| InvalidInstallation)?;
            if key != record.mutation.as_bytes() {
                return Err(InvalidInstallation);
            }
        }
        (MigratableRecord::Outbox, Some(Value::Outbox(_))) => {
            crate::mutation_records::decode_outbox(key, bytes).map_err(|_| InvalidInstallation)?;
        }
        (MigratableRecord::Idempotency, Some(Value::Idempotency(_))) => {
            crate::mutation_records::decode_idempotency(key, bytes)
                .map_err(|_| InvalidInstallation)?;
        }
        _ => return Err(InvalidInstallation),
    }
    stored.schema_version = 4;
    envelope_with(&stored.encode_to_vec(), sha256).map_err(|_| InvalidInstallation)
}

/// Verifies the v4 record and presents it with the same v3 checker codec.
/// This transformation is only in memory; it does not change the circuitry of the base.
pub(crate) fn checked_v4_record_as_v3(
    kind: MigratableRecord,
    key: &[u8],
    bytes: &[u8],
) -> Result<Vec<u8>, InvalidInstallation> {
    let payload = unenvelope_with(bytes, sha256).map_err(|_| InvalidInstallation)?;
    let mut stored = StoredValue::decode(payload.as_slice()).map_err(|_| InvalidInstallation)?;
    if stored.schema_version != 4 || stored.encode_to_vec() != payload {
        return Err(InvalidInstallation);
    }
    stored.schema_version = 3;
    let legacy = envelope_with(&stored.encode_to_vec(), sha256).map_err(|_| InvalidInstallation)?;
    if prepare_v3_record(kind, key, &legacy)? != bytes {
        return Err(InvalidInstallation);
    }
    Ok(legacy)
}

/// Checks the packet table in a single image before preparing the migration.
/// Does not check other tables, backup or no mutations.
#[allow(dead_code)] // This check cannot be considered as a permit for migration.
pub(crate) fn preflight_v3_packages(tx: &redb::ReadTransaction) -> Result<usize, SnapshotRefusal> {
    let meta = tx.open_table(META).map_err(|_| SnapshotRefusal::Corrupt)?;
    let marker = meta.get(SCHEMA_KEY).map_err(|_| SnapshotRefusal::Corrupt)?;
    if marker.as_ref().map(|value| value.value()) != Some(crate::redb::backend::SCHEMA_V3) {
        return Err(SnapshotRefusal::UnsupportedSchema);
    }
    match tx.open_table(INSTALLATIONS) {
        Err(redb::TableError::TableDoesNotExist(_)) => {}
        _ => return Err(SnapshotRefusal::Corrupt),
    }
    let table = tx
        .open_table(PACKAGES)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let mut count = 0usize;
    let mut package_ids = BTreeSet::new();
    let mut provider_ids = BTreeSet::new();
    let mut profile_ids = BTreeSet::new();
    for entry in table.iter().map_err(|_| SnapshotRefusal::Corrupt)? {
        let (key, value) = entry.map_err(|_| SnapshotRefusal::Corrupt)?;
        let prepared =
            prepare_v3_package(key.value(), value.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
        let package =
            decode_package(key.value(), &prepared).map_err(|_| SnapshotRefusal::Corrupt)?;
        if !package_ids.insert(package.package_id)
            || !provider_ids.insert(package.provider_id)
            || package
                .approved_profiles
                .into_iter()
                .any(|profile| !profile_ids.insert(profile.profile_id))
        {
            return Err(SnapshotRefusal::Corrupt);
        }
        count = count.checked_add(1).ok_or(SnapshotRefusal::Corrupt)?;
    }
    Ok(count)
}
