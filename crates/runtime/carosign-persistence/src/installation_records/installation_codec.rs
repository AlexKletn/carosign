use std::collections::BTreeSet;

use prost::Message;

use super::{
    InvalidInstallation, ProviderInstallationRecord, StoredValue, Value, envelope_with,
    record_fields::{id, profile_name, version},
    sha256, unenvelope_with,
};

fn path(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 1024
        && v.split('/').all(|p| {
            !p.is_empty()
                && p != "."
                && p != ".."
                && p.len() <= 255
                && !p.bytes().any(|b| b == 0 || b.is_ascii_control())
        })
}
pub(super) fn validate(
    key: &[u8],
    r: &ProviderInstallationRecord,
) -> Result<(), InvalidInstallation> {
    if key.len() != 32
        || !id(&r.package_id)
        || !id(&r.provider_id)
        || !id(&r.profile_id)
        || !id(&r.key_policy_id)
        || key[..16] != r.package_id
        || key[16..] != r.profile_id
        || r.revision == 0
        || r.inode == 0
        || !matches!(r.state, 1 | 2)
        || !matches!(
            (r.signer_key_version, r.signer_fingerprint.len()),
            (4, 20) | (5 | 6, 32)
        )
        || r.signer_fingerprint.iter().all(|b| *b == 0)
        || !path(&r.version_dir)
        || !version(&r.version, false)
        || !profile_name(&r.profile_name)
        || carosign_core::provider::PackageName::new(&r.package_name)
            .map(|p| p.as_str() != r.package_name)
            .unwrap_or(true)
        || r.files.len() < 4
        || r.files.len() > 128
    {
        return Err(InvalidInstallation);
    }
    let mut paths = BTreeSet::new();
    let mut roles = [0u8; 5];
    for f in &r.files {
        if !path(&f.path)
            || !paths.insert(&f.path)
            || f.verity_sha256.len() != 32
            || !matches!(f.role, 1..=5)
        {
            return Err(InvalidInstallation);
        };
        roles[(f.role - 1) as usize] = roles[(f.role - 1) as usize].saturating_add(1);
        let expected = match f.role {
            1 => Some(&r.policy_path),
            2 => Some(&r.manifest_path),
            3 => Some(&r.profile_path),
            4 => Some(&r.executable_path),
            _ => None,
        };
        if expected.is_some_and(|p| p != &f.path) {
            return Err(InvalidInstallation);
        }
    }
    if roles[..4] != [1, 1, 1, 1] {
        return Err(InvalidInstallation);
    }
    Ok(())
}
/// Encodes the verified record; the caller separately checks the system authority.
pub fn encode(
    key: &[u8],
    record: &ProviderInstallationRecord,
) -> Result<Vec<u8>, InvalidInstallation> {
    validate(key, record)?;
    let payload = StoredValue {
        schema_version: 4,
        value: Some(Value::ProviderInstallation(record.clone())),
    }
    .encode_to_vec();
    if payload.len() > 65536 {
        return Err(InvalidInstallation);
    }
    envelope_with(&payload, sha256).map_err(|_| InvalidInstallation)
}
/// Checks the shell, diagram, unknown fields and key mirrors.
pub fn decode(key: &[u8], bytes: &[u8]) -> Result<ProviderInstallationRecord, InvalidInstallation> {
    let payload = unenvelope_with(bytes, sha256).map_err(|_| InvalidInstallation)?;
    if payload.len() > 65536 {
        return Err(InvalidInstallation);
    }
    let value = StoredValue::decode(payload.as_slice()).map_err(|_| InvalidInstallation)?;
    if value.schema_version != 4 || value.encode_to_vec() != payload {
        return Err(InvalidInstallation);
    }
    let Some(Value::ProviderInstallation(record)) = value.value else {
        return Err(InvalidInstallation);
    };
    validate(key, &record)?;
    Ok(record)
}
