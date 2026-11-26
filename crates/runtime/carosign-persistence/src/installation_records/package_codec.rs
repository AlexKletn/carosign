use std::collections::BTreeSet;

use prost::Message;

use super::{
    InvalidInstallation, PackageRecord, StoredValue, Value,
    record_fields::{id, profile_name, version},
    sha256, unenvelope_with,
};

/// Checks the v4 packet record and its key before matching with the installation.
/// Input bytes and checksum are not proof of origin.
pub fn decode_package(key: &[u8], bytes: &[u8]) -> Result<PackageRecord, InvalidInstallation> {
    let payload = unenvelope_with(bytes, sha256).map_err(|_| InvalidInstallation)?;
    let value = StoredValue::decode(payload.as_slice()).map_err(|_| InvalidInstallation)?;
    if value.schema_version != 4 || value.encode_to_vec() != payload {
        return Err(InvalidInstallation);
    }
    let Some(Value::PackageRecord(package)) = value.value else {
        return Err(InvalidInstallation);
    };
    if carosign_core::provider::PackageName::new(&package.package_name)
        .map(|name| name.as_str().as_bytes() != key || name.as_str() != package.package_name)
        .unwrap_or(true)
        || !id(&package.package_id)
        || !id(&package.provider_id)
        || !version(&package.installed_version, true)
    {
        return Err(InvalidInstallation);
    }
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for profile in &package.approved_profiles {
        if !profile_name(&profile.profile_name)
            || !id(&profile.profile_id)
            || !names.insert(&profile.profile_name)
            || !ids.insert(&profile.profile_id)
        {
            return Err(InvalidInstallation);
        }
    }
    Ok(package)
}
