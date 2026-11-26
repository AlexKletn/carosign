use std::collections::BTreeSet;

use super::{
    PackageMismatch, PackageRecord, ProviderInstallationRecord, installation_codec::validate,
    record_fields::id,
};

/// Verifies the installation with a package read from the same transaction snapshot.
/// Verification does not confirm the origin, authenticity or relevance of the record.
pub fn check_package(
    key: &[u8],
    record: &ProviderInstallationRecord,
    package: &PackageRecord,
) -> Result<(), PackageMismatch> {
    validate(key, record).map_err(|_| PackageMismatch)?;
    if package.package_name != record.package_name
        || package.package_id != record.package_id
        || package.provider_id != record.provider_id
        || (package.installed_version != record.version
            && !(record.state == 2 && package.installed_version.is_empty()))
    {
        return Err(PackageMismatch);
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut found = false;
    for profile in &package.approved_profiles {
        if profile.profile_name.is_empty()
            || !id(&profile.profile_id)
            || !names.insert(&profile.profile_name)
            || !ids.insert(&profile.profile_id)
        {
            return Err(PackageMismatch);
        }
        if profile.profile_name == record.profile_name {
            if profile.profile_id != record.profile_id {
                return Err(PackageMismatch);
            }
            found = true;
        }
    }
    if !found {
        return Err(PackageMismatch);
    }
    Ok(())
}
