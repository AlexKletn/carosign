use redb::ReadableTable;

use super::{InvalidInstallation, PackageRecord, decode_package};

// The identifiers are allocated to one approved name and profile pair, and the second name
// cannot get the existing area just because the ID matches.
pub(super) fn check_unique_package_identity(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
    selected_key: &[u8],
    selected: &PackageRecord,
) -> Result<(), InvalidInstallation> {
    for item in table.iter().map_err(|_| InvalidInstallation)? {
        let (key, value) = item.map_err(|_| InvalidInstallation)?;
        if key.value() == selected_key {
            continue;
        }
        let other = decode_package(key.value(), value.value())?;
        if other.package_id == selected.package_id
            || other.provider_id == selected.provider_id
            || other.approved_profiles.iter().any(|profile| {
                selected
                    .approved_profiles
                    .iter()
                    .any(|current| current.profile_id == profile.profile_id)
            })
        {
            return Err(InvalidInstallation);
        }
    }
    Ok(())
}
