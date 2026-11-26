use super::{
    INSTALLATIONS, META, PACKAGES, PUBLICATION_PROVENANCE, PUBLICATION_QUARANTINE, PackageRecord,
    ProviderInstallationRecord, SCHEMA_KEY, SCHEMA_V4, check_package,
    check_unique_package_identity, decode, decode_package,
};
use redb::ReadableTable;

// An unknown metadate and an unfinished log prohibit the use of the image.
// The completed journal remains in the database according to the adopted migration agreement.
pub(super) fn normal_v4_meta(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
) -> Result<bool, ()> {
    let marker = table.get(SCHEMA_KEY).map_err(|_| ())?;
    if marker.as_ref().map(|value| value.value()) != Some(SCHEMA_V4) {
        return Ok(false);
    }
    for entry in table.iter().map_err(|_| ())? {
        let (key, value) = entry.map_err(|_| ())?;
        if key.value() != SCHEMA_KEY
            && !crate::migration::is_committed_v4_journal(key.value(), value.value())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Reads both entries and a token from a single open read transaction.
/// Does not create tables, does not confirm authority and does not allow launch.
pub fn read_active_snapshot(
    tx: &redb::ReadTransaction,
    package_key: &[u8],
    installation_key: &[u8],
) -> Result<ProviderInstallationRecord, SnapshotRefusal> {
    let meta = tx.open_table(META).map_err(|_| SnapshotRefusal::Corrupt)?;
    if !normal_v4_meta(&meta).map_err(|_| SnapshotRefusal::Corrupt)? {
        return Err(SnapshotRefusal::UnsupportedSchema);
    }
    let packages = tx
        .open_table(PACKAGES)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let installations = tx
        .open_table(INSTALLATIONS)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let package = packages
        .get(package_key)
        .map_err(|_| SnapshotRefusal::Corrupt)?
        .ok_or(SnapshotRefusal::NotFound)?;
    let installation = installations
        .get(installation_key)
        .map_err(|_| SnapshotRefusal::Corrupt)?
        .ok_or(SnapshotRefusal::NotFound)?;
    let package =
        decode_package(package_key, package.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
    check_unique_package_identity(&packages, package_key, &package)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let installation =
        decode(installation_key, installation.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
    check_package(installation_key, &installation, &package)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    if installation.state != 1 {
        return Err(SnapshotRefusal::Revoked);
    }
    Ok(installation)
}

#[allow(dead_code)]
pub(crate) fn read_quarantined_snapshot(
    tx: &redb::ReadTransaction,
    package_key: &[u8],
    installation_key: &[u8],
) -> Result<QuarantinedSnapshot, SnapshotRefusal> {
    let meta = tx.open_table(META).map_err(|_| SnapshotRefusal::Corrupt)?;
    let mut seen_schema = false;
    let mut seen_quarantine = false;
    let mut provenance = None;
    for entry in meta.iter().map_err(|_| SnapshotRefusal::Corrupt)? {
        let (key, value) = entry.map_err(|_| SnapshotRefusal::Corrupt)?;
        match key.value() {
            SCHEMA_KEY if value.value() == SCHEMA_V4 => seen_schema = true,
            PUBLICATION_QUARANTINE if value.value() == installation_key => {
                seen_quarantine = true;
            }
            PUBLICATION_PROVENANCE => {
                provenance = Some(
                    value
                        .value()
                        .try_into()
                        .map_err(|_| SnapshotRefusal::Corrupt)?,
                );
            }
            k if crate::migration::is_committed_v4_journal(k, value.value()) => {}
            _ => return Err(SnapshotRefusal::UnsupportedSchema),
        }
    }
    if !seen_schema || !seen_quarantine {
        return Err(SnapshotRefusal::UnsupportedSchema);
    }
    // Recovery should not bypass the general registry check: a damaged adjacent record or
    // broken communication makes the entire shot unusable.
    validate_v4_snapshot(tx)?;
    let packages = tx
        .open_table(PACKAGES)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let installations = tx
        .open_table(INSTALLATIONS)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let package = packages
        .get(package_key)
        .map_err(|_| SnapshotRefusal::Corrupt)?
        .ok_or(SnapshotRefusal::NotFound)?;
    let package =
        decode_package(package_key, package.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
    check_unique_package_identity(&packages, package_key, &package)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let installation = installations
        .get(installation_key)
        .map_err(|_| SnapshotRefusal::Corrupt)?
        .ok_or(SnapshotRefusal::NotFound)?;
    let installation =
        decode(installation_key, installation.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
    check_package(installation_key, &installation, &package)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    if installation.state != 1 {
        return Err(SnapshotRefusal::Revoked);
    }
    Ok(QuarantinedSnapshot {
        package,
        installation,
        provenance,
    })
}

/// Re-reads the quarantine in the new image after external file check.
/// The match itself does not confirm the origin, does not serialize the review with
/// subsequent recording and does not allow quarantine or run the worker.
#[allow(dead_code)]
pub(crate) fn revalidate_quarantined_snapshot(
    database: &redb::Database,
    package_key: &[u8],
    installation_key: &[u8],
    prepared: &QuarantinedSnapshot,
) -> Result<(), SnapshotRefusal> {
    let fresh_tx = database
        .begin_read()
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let current = read_quarantined_snapshot(&fresh_tx, package_key, installation_key)?;
    if &current != prepared {
        return Err(SnapshotRefusal::Changed);
    }
    Ok(())
}

/// Opens a new image and cross-checks the entire prepared recording, including the audit and
/// measurements, and the system policy is checked by a separate source.
pub fn revalidate_snapshot(
    database: &redb::Database,
    package_key: &[u8],
    installation_key: &[u8],
    prepared: &ProviderInstallationRecord,
) -> Result<(), SnapshotRefusal> {
    let fresh_tx = database
        .begin_read()
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let current = read_active_snapshot(&fresh_tx, package_key, installation_key)?;
    if &current != prepared {
        return Err(SnapshotRefusal::Changed);
    }
    Ok(())
}

/// Checks all packages and installations of one image, without confirming trust in them.
pub(crate) fn validate_v4_snapshot(tx: &redb::ReadTransaction) -> Result<(), SnapshotRefusal> {
    let packages = tx
        .open_table(PACKAGES)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    let installations = tx
        .open_table(INSTALLATIONS)
        .map_err(|_| SnapshotRefusal::Corrupt)?;
    for item in packages.iter().map_err(|_| SnapshotRefusal::Corrupt)? {
        let (key, value) = item.map_err(|_| SnapshotRefusal::Corrupt)?;
        let package =
            decode_package(key.value(), value.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
        check_unique_package_identity(&packages, key.value(), &package)
            .map_err(|_| SnapshotRefusal::Corrupt)?;
    }
    for item in installations.iter().map_err(|_| SnapshotRefusal::Corrupt)? {
        let (key, value) = item.map_err(|_| SnapshotRefusal::Corrupt)?;
        let installation =
            decode(key.value(), value.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
        let package_key = installation.package_name.as_bytes();
        let package = packages
            .get(package_key)
            .map_err(|_| SnapshotRefusal::Corrupt)?
            .ok_or(SnapshotRefusal::Corrupt)?;
        let package =
            decode_package(package_key, package.value()).map_err(|_| SnapshotRefusal::Corrupt)?;
        check_package(key.value(), &installation, &package)
            .map_err(|_| SnapshotRefusal::Corrupt)?;
    }
    Ok(())
}

mod snapshot_refusal;
pub use snapshot_refusal::SnapshotRefusal;
mod quarantined_snapshot;
pub use quarantined_snapshot::QuarantinedSnapshot;
