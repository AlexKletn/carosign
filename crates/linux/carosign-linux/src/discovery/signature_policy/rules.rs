use super::PolicyError;
use std::collections::BTreeMap;

pub(super) fn digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let hex = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(hex, 16).ok()?;
    }
    Some(out)
}

fn runtime_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.starts_with('/')
        && value.split('/').all(|p| {
            !p.is_empty()
                && p != "."
                && p != ".."
                && p.len() <= 255
                && !p.bytes().any(|b| b == 0 || b.is_ascii_control())
        })
}

pub(super) struct Rules {
    pub(super) policy_id: [u8; 16],
    pub(super) primary: String,
    pub(super) keyring: [u8; 32],
    pub(super) verifier: [u8; 32],
    pub(super) runtime: BTreeMap<String, [u8; 32]>,
}

impl Rules {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, PolicyError> {
        let text = std::str::from_utf8(bytes).map_err(|_| PolicyError::Invalid)?;
        let mut table = text
            .parse::<toml::Table>()
            .map_err(|_| PolicyError::Invalid)?;
        if table.remove("schema").and_then(|v| v.as_integer()) != Some(1) {
            return Err(PolicyError::Invalid);
        }
        fn string(table: &mut toml::Table, key: &str) -> Result<String, PolicyError> {
            table
                .remove(key)
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or(PolicyError::Invalid)
        }
        let policy_id_hex = string(&mut table, "key_policy_id")?;
        if policy_id_hex.len() != 32 || !policy_id_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(PolicyError::Invalid);
        }
        let mut policy_id = [0u8; 16];
        for (out, chunk) in policy_id
            .iter_mut()
            .zip(policy_id_hex.as_bytes().chunks_exact(2))
        {
            let hex = std::str::from_utf8(chunk).map_err(|_| PolicyError::Invalid)?;
            *out = u8::from_str_radix(hex, 16).map_err(|_| PolicyError::Invalid)?;
        }
        if policy_id == [0; 16] {
            return Err(PolicyError::Invalid);
        }
        let primary = string(&mut table, "primary_fingerprint")?;
        if !matches!(primary.len(), 40 | 64)
            || !primary
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        {
            return Err(PolicyError::Invalid);
        }
        let keyring = digest(&string(&mut table, "keyring_verity")?).ok_or(PolicyError::Invalid)?;
        let verifier =
            digest(&string(&mut table, "verifier_verity")?).ok_or(PolicyError::Invalid)?;
        let runtime_values = table
            .remove("runtime")
            .and_then(|v| v.as_array().cloned())
            .ok_or(PolicyError::Invalid)?;
        if !table.is_empty() || runtime_values.is_empty() || runtime_values.len() > 128 {
            return Err(PolicyError::Invalid);
        }
        let mut runtime = BTreeMap::new();
        for value in runtime_values {
            let mut item = value.as_table().cloned().ok_or(PolicyError::Invalid)?;
            let name = string(&mut item, "path")?;
            let measured = digest(&string(&mut item, "verity")?).ok_or(PolicyError::Invalid)?;
            if !item.is_empty() || !runtime_path(&name) || runtime.insert(name, measured).is_some()
            {
                return Err(PolicyError::Invalid);
            }
        }
        if runtime.get("usr/bin/gpgv") != Some(&verifier) {
            return Err(PolicyError::Invalid);
        }
        Ok(Self {
            policy_id,
            primary,
            keyring,
            verifier,
            runtime,
        })
    }
}
