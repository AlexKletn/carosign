use super::{
    INSTALLATIONS, META, PACKAGES, StoredValue, Value, check_package, decode, decode_package,
    encode, envelope_with, normal_v4_meta, sha256,
};
use prost::Message;
use redb::{ReadableTable, TableHandle};
use std::collections::BTreeSet;

/// The recall is not confirmed until the caller has recorded the transaction.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RevokeFailure {
    UnsupportedSchema,
    Corrupt,
    NotFound,
    Conflict,
    RevisionExhausted,
}

/// Prepares feedback in an existing write transaction. Installer authority, sustained
/// commitment and response to ambiguous outcome is provided by the owner.
/// In case of an error, the function does not change the data or create missing tables.
#[allow(dead_code)] // Waiting for the system owner of the installation.
pub(crate) fn stage_revocation(
    tx: &redb::WriteTransaction,
    package_key: &[u8],
    installation_key: &[u8],
    expected_revision: u64,
) -> Result<(), RevokeFailure> {
    let names: BTreeSet<String> = tx
        .list_tables()
        .map_err(|_| RevokeFailure::Corrupt)?
        .map(|table| table.name().to_owned())
        .collect();
    if ["meta", "packages", "provider_installations"]
        .iter()
        .any(|name| !names.contains(*name))
    {
        return Err(RevokeFailure::Corrupt);
    }
    let meta = tx.open_table(META).map_err(|_| RevokeFailure::Corrupt)?;
    if !normal_v4_meta(&meta).map_err(|_| RevokeFailure::Corrupt)? {
        return Err(RevokeFailure::UnsupportedSchema);
    }
    let packages = tx
        .open_table(PACKAGES)
        .map_err(|_| RevokeFailure::Corrupt)?;
    let package = packages
        .get(package_key)
        .map_err(|_| RevokeFailure::Corrupt)?
        .ok_or(RevokeFailure::NotFound)?;
    let package =
        decode_package(package_key, package.value()).map_err(|_| RevokeFailure::Corrupt)?;
    let mut installations = tx
        .open_table(INSTALLATIONS)
        .map_err(|_| RevokeFailure::Corrupt)?;
    let encoded = installations
        .get(installation_key)
        .map_err(|_| RevokeFailure::Corrupt)?
        .ok_or(RevokeFailure::NotFound)?;
    let mut installation =
        decode(installation_key, encoded.value()).map_err(|_| RevokeFailure::Corrupt)?;
    drop(encoded);
    check_package(installation_key, &installation, &package).map_err(|_| RevokeFailure::Corrupt)?;
    if installation.state != 1 || installation.revision != expected_revision {
        return Err(RevokeFailure::Conflict);
    }
    installation.revision = installation
        .revision
        .checked_add(1)
        .ok_or(RevokeFailure::RevisionExhausted)?;
    installation.state = 2;
    let output = encode(installation_key, &installation).map_err(|_| RevokeFailure::Corrupt)?;
    installations
        .insert(installation_key, output.as_slice())
        .map_err(|_| RevokeFailure::Corrupt)?;
    Ok(())
}

/// Prepares to delete the version of the package and recall all its existing profiles.
///
/// The caller is obliged to verify the authority and, in case of any recording error, cancel
/// the transaction. Success does not confirm a steady commit, recall of processes or release
/// of resources.
#[allow(dead_code)]
pub(crate) fn stage_uninstall(
    tx: &redb::WriteTransaction,
    package_key: &[u8],
    expected_active: &[(carosign_core::ids::ProfileId, u64)],
) -> Result<(), RevokeFailure> {
    let names: BTreeSet<String> = tx
        .list_tables()
        .map_err(|_| RevokeFailure::Corrupt)?
        .map(|table| table.name().to_owned())
        .collect();
    if ["meta", "packages", "provider_installations"]
        .iter()
        .any(|name| !names.contains(*name))
    {
        return Err(RevokeFailure::Corrupt);
    }
    let meta = tx.open_table(META).map_err(|_| RevokeFailure::Corrupt)?;
    if !normal_v4_meta(&meta).map_err(|_| RevokeFailure::Corrupt)? {
        return Err(RevokeFailure::UnsupportedSchema);
    }
    let mut packages = tx
        .open_table(PACKAGES)
        .map_err(|_| RevokeFailure::Corrupt)?;
    let encoded = packages
        .get(package_key)
        .map_err(|_| RevokeFailure::Corrupt)?
        .ok_or(RevokeFailure::NotFound)?;
    let mut package =
        decode_package(package_key, encoded.value()).map_err(|_| RevokeFailure::Corrupt)?;
    drop(encoded);
    if package.installed_version.is_empty() {
        return Err(RevokeFailure::Conflict);
    }
    let mut expected = std::collections::BTreeMap::new();
    for (profile, revision) in expected_active {
        if *revision == 0 || expected.insert(*profile.as_bytes(), *revision).is_some() {
            return Err(RevokeFailure::Conflict);
        }
    }
    let mut installations = tx
        .open_table(INSTALLATIONS)
        .map_err(|_| RevokeFailure::Corrupt)?;
    let mut changes = Vec::new();
    let mut found_installation = false;
    {
        let entries = installations.iter().map_err(|_| RevokeFailure::Corrupt)?;
        for item in entries {
            let (key, value) = item.map_err(|_| RevokeFailure::Corrupt)?;
            let key = key.value();
            if key.len() != 32 {
                return Err(RevokeFailure::Corrupt);
            }
            if key[..16] != package.package_id {
                continue;
            }
            found_installation = true;
            let mut record = decode(key, value.value()).map_err(|_| RevokeFailure::Corrupt)?;
            check_package(key, &record, &package).map_err(|_| RevokeFailure::Corrupt)?;
            if record.state == 1 {
                let profile: [u8; 16] = record
                    .profile_id
                    .as_slice()
                    .try_into()
                    .map_err(|_| RevokeFailure::Corrupt)?;
                if expected.remove(&profile) != Some(record.revision) {
                    return Err(RevokeFailure::Conflict);
                }
                record.revision = record
                    .revision
                    .checked_add(1)
                    .ok_or(RevokeFailure::RevisionExhausted)?;
                record.state = 2;
                let output = encode(key, &record).map_err(|_| RevokeFailure::Corrupt)?;
                changes.push((key.to_vec(), output));
            }
        }
    }
    if !expected.is_empty() || !found_installation {
        return Err(RevokeFailure::Conflict);
    }
    package.installed_version.clear();
    let package_bytes = envelope_with(
        &StoredValue {
            schema_version: 4,
            value: Some(Value::PackageRecord(package)),
        }
        .encode_to_vec(),
        sha256,
    )
    .map_err(|_| RevokeFailure::Corrupt)?;
    decode_package(package_key, &package_bytes).map_err(|_| RevokeFailure::Corrupt)?;
    for (key, value) in &changes {
        installations
            .insert(key.as_slice(), value.as_slice())
            .map_err(|_| RevokeFailure::Corrupt)?;
    }
    packages
        .insert(package_key, package_bytes.as_slice())
        .map_err(|_| RevokeFailure::Corrupt)?;
    Ok(())
}
