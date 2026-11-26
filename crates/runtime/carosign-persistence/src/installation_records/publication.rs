use super::{
    INSTALLATIONS, META, PACKAGES, PUBLICATION_PROVENANCE, PUBLICATION_QUARANTINE, PackageRecord,
    ProviderInstallationRecord, SCHEMA_KEY, SCHEMA_V5, StoredValue, Value, check_package,
    check_unique_package_identity, decode, decode_package, encode, envelope_with, normal_v4_meta,
    sha256,
};
use prost::Message;
use redb::{ReadableTable, ReadableTableMetadata, TableHandle};
use std::collections::BTreeSet;

/// The preparation of the record does not confirm either the signature or the authority of
/// the installer.
#[derive(Debug, PartialEq, Eq)]
pub enum PublicationFailure {
    UnsupportedSchema,
    Corrupt,
    NotFound,
    Conflict,
    RevisionExhausted,
    /// Policy surveillance stopped before fixation; transaction cancelled.
    PolicyChanged,
    /// The outcome of the fixation is unknown; repeating and issuing the clearance is
    /// prohibited.
    CommitUnknown,
    /// Records are preserved with quarantine; independent restoration is required.
    RecoveryRequired,
}

/// The first publication of a package and installation in a single stable transaction.
///
/// This is only the storage boundary: before calling, the individual system owner must
/// certify the signature, policy, origin of the files and authority.
/// With an unknown outcome of fixation, the installation cannot be considered confirmed.
#[allow(dead_code)]
pub(crate) fn publish_initial_package_and_installation(
    database: &redb::Database,
    package: &PackageRecord,
    installation_key: &[u8],
    candidate: &ProviderInstallationRecord,
) -> Result<(), PublicationFailure> {
    publish_initial_with_policy_check(database, package, installation_key, candidate, || true)
}

/// Preliminary testing of policies around sustainable fixation.
/// Return of success is not a launch clearance: for it, the caller is obliged to separately
/// link the system source, review observation and supervisor.
/// When you retract after you start commit, the state is considered uncertain, not permission
/// to repeat the publication or launch.
#[allow(dead_code)]
pub(crate) fn publish_initial_with_policy_check(
    database: &redb::Database,
    package: &PackageRecord,
    installation_key: &[u8],
    candidate: &ProviderInstallationRecord,
    policy_unchanged: impl FnMut() -> bool,
) -> Result<(), PublicationFailure> {
    publish_initial_with_provenance(
        database,
        package,
        installation_key,
        candidate,
        None,
        policy_unchanged,
    )
}

pub(crate) fn publish_initial_with_provenance(
    database: &redb::Database,
    package: &PackageRecord,
    installation_key: &[u8],
    candidate: &ProviderInstallationRecord,
    provenance: Option<[[u8; 32]; 2]>,
    mut policy_unchanged: impl FnMut() -> bool,
) -> Result<(), PublicationFailure> {
    if !policy_unchanged() {
        return Err(PublicationFailure::PolicyChanged);
    }
    let package_key = package.package_name.as_bytes();
    let payload = StoredValue {
        schema_version: 4,
        value: Some(Value::PackageRecord(package.clone())),
    }
    .encode_to_vec();
    let encoded = envelope_with(&payload, sha256).map_err(|_| PublicationFailure::Corrupt)?;
    decode_package(package_key, &encoded).map_err(|_| PublicationFailure::Conflict)?;
    let tx = database
        .begin_write()
        .map_err(|_| PublicationFailure::Corrupt)?;
    {
        // open_table In a written transaction, a missing table is created; an incomplete
        // diagram cannot be silently “corrected” when published.
        let names: BTreeSet<String> = tx
            .list_tables()
            .map_err(|_| PublicationFailure::Corrupt)?
            .map(|table| table.name().to_owned())
            .collect();
        if ["meta", "packages", "provider_installations"]
            .iter()
            .any(|name| !names.contains(*name))
        {
            return Err(PublicationFailure::Corrupt);
        }
        let meta = tx
            .open_table(META)
            .map_err(|_| PublicationFailure::Corrupt)?;
        if !normal_v4_meta(&meta).map_err(|_| PublicationFailure::Corrupt)? {
            return Err(PublicationFailure::UnsupportedSchema);
        }
        let mut packages = tx
            .open_table(PACKAGES)
            .map_err(|_| PublicationFailure::Corrupt)?;
        // The new installation should not be fixed on top of an already damaged registry: the
        // presence of an agreed candidate does not confirm the rest of the entries of the
        // same image.
        let installations = tx
            .open_table(INSTALLATIONS)
            .map_err(|_| PublicationFailure::Corrupt)?;
        for item in installations
            .iter()
            .map_err(|_| PublicationFailure::Corrupt)?
        {
            let (key, value) = item.map_err(|_| PublicationFailure::Corrupt)?;
            let existing =
                decode(key.value(), value.value()).map_err(|_| PublicationFailure::Corrupt)?;
            let existing_key = existing.package_name.as_bytes();
            let existing_package = packages
                .get(existing_key)
                .map_err(|_| PublicationFailure::Corrupt)?
                .ok_or(PublicationFailure::Corrupt)?;
            let existing_package = decode_package(existing_key, existing_package.value())
                .map_err(|_| PublicationFailure::Corrupt)?;
            check_package(key.value(), &existing, &existing_package)
                .map_err(|_| PublicationFailure::Corrupt)?;
        }
        drop(installations);
        if packages
            .get(package_key)
            .map_err(|_| PublicationFailure::Corrupt)?
            .is_some()
        {
            return Err(PublicationFailure::Conflict);
        }
        check_unique_package_identity(&packages, package_key, package)
            .map_err(|_| PublicationFailure::Conflict)?;
        packages
            .insert(package_key, encoded.as_slice())
            .map_err(|_| PublicationFailure::Corrupt)?;
    }
    // In any error, tx is destroyed without commit; no partial packet remains.
    stage_publication(&tx, package_key, installation_key, candidate, None)?;
    if !policy_unchanged() {
        return Err(PublicationFailure::PolicyChanged);
    }
    {
        let mut meta = tx
            .open_table(META)
            .map_err(|_| PublicationFailure::Corrupt)?;
        meta.insert(PUBLICATION_QUARANTINE, installation_key)
            .map_err(|_| PublicationFailure::Corrupt)?;
        if let Some([archive, signature]) = provenance {
            let mut measurements = [0u8; 64];
            measurements[..32].copy_from_slice(&archive);
            measurements[32..].copy_from_slice(&signature);
            meta.insert(PUBLICATION_PROVENANCE, measurements.as_slice())
                .map_err(|_| PublicationFailure::Corrupt)?;
        }
    }
    tx.commit().map_err(|_| PublicationFailure::CommitUnknown)?;
    if !policy_unchanged() {
        return Err(PublicationFailure::CommitUnknown);
    }
    // Even a successful re-check does not allow the barrier to be removed: commit a separate
    // withdrawal transaction may result in an unknown outcome, and after a restart, there is
    // no independent source of origin.
    // Therefore, quarantine is maintained until the implementation of verified restoration.
    Err(PublicationFailure::RecoveryRequired)
}

/// Prepares the publication of an already verified installation in a common transaction.
/// The caller is required to confirm origin and authority, record the transaction
/// sustainably, and handle an ambiguous outcome before issuing an admission.
/// The package must already match the candidate version in this transaction.
#[allow(dead_code)]
pub(crate) fn stage_publication(
    tx: &redb::WriteTransaction,
    package_key: &[u8],
    installation_key: &[u8],
    candidate: &ProviderInstallationRecord,
    expected_revision: Option<u64>,
) -> Result<(), PublicationFailure> {
    stage_publication_for_schema(
        tx,
        package_key,
        installation_key,
        candidate,
        expected_revision,
        false,
    )
}

/// Prepares publication only within the verified transaction of the primary scheme.
#[allow(dead_code)]
pub(crate) fn stage_publication_v5(
    tx: &mut crate::redb::backend::GuardedWriteTx<'_>,
    package_key: &[u8],
    installation_key: &[u8],
    candidate: &ProviderInstallationRecord,
    expected_revision: Option<u64>,
) -> Result<(), PublicationFailure> {
    let tx = tx
        .installation_transaction()
        .map_err(|_| PublicationFailure::Corrupt)?;
    stage_publication_for_schema(
        tx,
        package_key,
        installation_key,
        candidate,
        expected_revision,
        true,
    )
}

fn stage_publication_for_schema(
    tx: &redb::WriteTransaction,
    package_key: &[u8],
    installation_key: &[u8],
    candidate: &ProviderInstallationRecord,
    expected_revision: Option<u64>,
    initial_v5: bool,
) -> Result<(), PublicationFailure> {
    let names: BTreeSet<String> = tx
        .list_tables()
        .map_err(|_| PublicationFailure::Corrupt)?
        .map(|table| table.name().to_owned())
        .collect();
    if ["meta", "packages", "provider_installations"]
        .iter()
        .any(|name| !names.contains(*name))
    {
        return Err(PublicationFailure::Corrupt);
    }
    let meta = tx
        .open_table(META)
        .map_err(|_| PublicationFailure::Corrupt)?;
    let valid_meta = if initial_v5 {
        meta.len().map_err(|_| PublicationFailure::Corrupt)? == 1
            && meta
                .get(SCHEMA_KEY)
                .map_err(|_| PublicationFailure::Corrupt)?
                .is_some_and(|value| value.value() == SCHEMA_V5)
    } else {
        normal_v4_meta(&meta).map_err(|_| PublicationFailure::Corrupt)?
    };
    if !valid_meta {
        return Err(PublicationFailure::UnsupportedSchema);
    }
    let packages = tx
        .open_table(PACKAGES)
        .map_err(|_| PublicationFailure::Corrupt)?;
    let package = packages
        .get(package_key)
        .map_err(|_| PublicationFailure::Corrupt)?
        .ok_or(PublicationFailure::NotFound)?;
    let package =
        decode_package(package_key, package.value()).map_err(|_| PublicationFailure::Corrupt)?;
    check_unique_package_identity(&packages, package_key, &package)
        .map_err(|_| PublicationFailure::Corrupt)?;
    if candidate.state != 1 || check_package(installation_key, candidate, &package).is_err() {
        return Err(PublicationFailure::Conflict);
    }
    let output = encode(installation_key, candidate).map_err(|_| PublicationFailure::Corrupt)?;
    let mut installations = tx
        .open_table(INSTALLATIONS)
        .map_err(|_| PublicationFailure::Corrupt)?;
    let old = installations
        .get(installation_key)
        .map_err(|_| PublicationFailure::Corrupt)?
        .map(|v| decode(installation_key, v.value()).map_err(|_| PublicationFailure::Corrupt))
        .transpose()?;
    match (old, expected_revision) {
        (None, None) if candidate.revision == 1 => {}
        (Some(old), Some(expected)) if old.revision == expected => {
            let next = expected
                .checked_add(1)
                .ok_or(PublicationFailure::RevisionExhausted)?;
            if candidate.revision != next
                || old.package_id != candidate.package_id
                || old.provider_id != candidate.provider_id
                || old.profile_id != candidate.profile_id
                || old.package_name != candidate.package_name
                || old.profile_name != candidate.profile_name
                || old.signer_key_version != candidate.signer_key_version
                || old.signer_fingerprint != candidate.signer_fingerprint
                || old.key_policy_id != candidate.key_policy_id
            {
                return Err(PublicationFailure::Conflict);
            }
        }
        _ => return Err(PublicationFailure::Conflict),
    }
    installations
        .insert(installation_key, output.as_slice())
        .map_err(|_| PublicationFailure::Corrupt)?;
    Ok(())
}
