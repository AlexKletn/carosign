//! Test prototype of a separate installer record registry.
//!
//! The entry here doesn't prove the installer's signature or authority. It's only compiled in
//! tests and not connected to the service. Specifically, it doesn't provide a single
//! transactional domain with a single transactional domain. `packages` 01 and therefore
//! cannot be used as a system source.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};

const RECORDS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("installation_records_v1");
const MAX_RECORD: usize = 262_144;
const MAX_FILES: usize = 128;

fn key(record: &InstallationRecord) -> [u8; 48] {
    let mut result = [0; 48];
    result[..16].copy_from_slice(&record.package_id);
    result[16..32].copy_from_slice(&record.provider_id);
    result[32..].copy_from_slice(&record.profile_id);
    result
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 255
                && !part.bytes().any(|byte| byte.is_ascii_control())
        })
}

fn encode(record: &InstallationRecord) -> Result<Vec<u8>, LedgerError> {
    if [&record.package_id, &record.provider_id, &record.profile_id]
        .iter()
        .any(|id| id.iter().all(|byte| *byte == 0))
        || record.root_inode == 0
        || record.revision == 0
        || !matches!(record.fingerprint.len(), 40 | 64)
        || !record
            .fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        || record.files.is_empty()
        || record.files.len() > MAX_FILES
        || record.files.keys().any(|path| !valid_path(path))
    {
        return Err(LedgerError::Invalid);
    }
    let mut bytes = b"BIL1".to_vec();
    bytes.extend_from_slice(&key(record));
    bytes.extend_from_slice(&record.root_device.to_be_bytes());
    bytes.extend_from_slice(&record.root_inode.to_be_bytes());
    bytes.extend_from_slice(&record.revision.to_be_bytes());
    bytes.push(u8::from(record.revoked));
    bytes.push(record.fingerprint.len() as u8);
    bytes.extend_from_slice(record.fingerprint.as_bytes());
    bytes.extend_from_slice(&(record.files.len() as u16).to_be_bytes());
    for (path, digest) in &record.files {
        bytes.extend_from_slice(&(path.len() as u16).to_be_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(digest);
    }
    if bytes.len() > MAX_RECORD {
        return Err(LedgerError::Invalid);
    }
    Ok(bytes)
}

fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8], LedgerError> {
    if input.len() < n {
        return Err(LedgerError::Corrupt);
    }
    let (head, tail) = input.split_at(n);
    *input = tail;
    Ok(head)
}

fn decode(expected_key: &[u8], mut bytes: &[u8]) -> Result<InstallationRecord, LedgerError> {
    if bytes.len() > MAX_RECORD || take(&mut bytes, 4)? != b"BIL1" {
        return Err(LedgerError::Corrupt);
    }
    let stored_key = take(&mut bytes, 48)?;
    if stored_key != expected_key {
        return Err(LedgerError::Corrupt);
    }
    let package_id = stored_key[..16]
        .try_into()
        .map_err(|_| LedgerError::Corrupt)?;
    let provider_id = stored_key[16..32]
        .try_into()
        .map_err(|_| LedgerError::Corrupt)?;
    let profile_id = stored_key[32..]
        .try_into()
        .map_err(|_| LedgerError::Corrupt)?;
    let root_device = u64::from_be_bytes(
        take(&mut bytes, 8)?
            .try_into()
            .map_err(|_| LedgerError::Corrupt)?,
    );
    let root_inode = u64::from_be_bytes(
        take(&mut bytes, 8)?
            .try_into()
            .map_err(|_| LedgerError::Corrupt)?,
    );
    let revision = u64::from_be_bytes(
        take(&mut bytes, 8)?
            .try_into()
            .map_err(|_| LedgerError::Corrupt)?,
    );
    let revoked = match take(&mut bytes, 1)?[0] {
        0 => false,
        1 => true,
        _ => return Err(LedgerError::Corrupt),
    };
    let fp_len = take(&mut bytes, 1)?[0] as usize;
    let fingerprint = std::str::from_utf8(take(&mut bytes, fp_len)?)
        .map_err(|_| LedgerError::Corrupt)?
        .to_owned();
    let count = u16::from_be_bytes(
        take(&mut bytes, 2)?
            .try_into()
            .map_err(|_| LedgerError::Corrupt)?,
    ) as usize;
    if count == 0 || count > MAX_FILES {
        return Err(LedgerError::Corrupt);
    }
    let mut files = BTreeMap::new();
    let mut previous: Option<String> = None;
    for _ in 0..count {
        let len = u16::from_be_bytes(
            take(&mut bytes, 2)?
                .try_into()
                .map_err(|_| LedgerError::Corrupt)?,
        ) as usize;
        let path = std::str::from_utf8(take(&mut bytes, len)?)
            .map_err(|_| LedgerError::Corrupt)?
            .to_owned();
        let digest = take(&mut bytes, 32)?
            .try_into()
            .map_err(|_| LedgerError::Corrupt)?;
        if previous.as_ref().is_some_and(|last| last >= &path) {
            return Err(LedgerError::Corrupt);
        }
        previous = Some(path.clone());
        files.insert(path, digest);
    }
    if !bytes.is_empty() {
        return Err(LedgerError::Corrupt);
    }
    let result = InstallationRecord {
        package_id,
        provider_id,
        profile_id,
        root_device,
        root_inode,
        fingerprint,
        revision,
        revoked,
        files,
    };
    encode(&result).map_err(|_| LedgerError::Corrupt)?;
    Ok(result)
}

#[path = "installation_ledger.tests.rs"]
#[cfg(test)]
mod tests;

mod ledger_error;
pub(crate) use ledger_error::LedgerError;
mod installation_record;
pub(crate) use installation_record::InstallationRecord;
mod ledger;
pub(crate) use ledger::Ledger;
