use super::{ENVELOPE_MAGIC, RecordError, StorageError, checksum, limits};

/// Takes off the shell and checks the amount.
pub(super) fn unenvelope(bytes: &[u8]) -> Result<Vec<u8>, RecordError> {
    unenvelope_with(bytes, checksum)
}

pub(crate) fn unenvelope_with(
    bytes: &[u8],
    digest: fn(&[u8]) -> [u8; 32],
) -> Result<Vec<u8>, RecordError> {
    if bytes.len() < 8 + limits::DIGEST_LEN {
        return Err(RecordError::Envelope(StorageError::Corrupt {
            what: "оболочка короче заголовка",
        }));
    }
    if bytes[..4] != ENVELOPE_MAGIC {
        return Err(RecordError::Envelope(StorageError::Corrupt {
            what: "магия оболочки",
        }));
    }
    let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if len == 0 || len > limits::STORED_VALUE_MAX {
        return Err(RecordError::Envelope(StorageError::LimitExceeded {
            limit: "StoredValue",
            actual: len,
            allowed: limits::STORED_VALUE_MAX,
        }));
    }
    if bytes.len() != 8 + len + limits::DIGEST_LEN {
        return Err(RecordError::Envelope(StorageError::Corrupt {
            what: "длина оболочки",
        }));
    }
    let payload = &bytes[8..8 + len];
    if bytes[8 + len..] != digest(payload) {
        return Err(RecordError::Envelope(StorageError::Corrupt {
            what: "контрольная сумма",
        }));
    }
    check_stored_value_assignment(payload)?;
    Ok(payload.to_vec())
}

fn check_stored_value_assignment(mut bytes: &[u8]) -> Result<(), RecordError> {
    use prost::encoding::{DecodeContext, decode_key, skip_field};
    let corrupt = || {
        RecordError::Envelope(StorageError::Corrupt {
            what: "неоднозначный StoredValue",
        })
    };
    let mut found = false;
    while !bytes.is_empty() {
        let (tag, wire) = decode_key(&mut bytes).map_err(|_| corrupt())?;
        if matches!(tag, 10..=18) {
            if found {
                return Err(corrupt());
            }
            found = true;
        }
        skip_field(wire, tag, &mut bytes, DecodeContext::default()).map_err(|_| corrupt())?;
    }
    Ok(())
}

/// Puts on a shell.
pub(super) fn envelope(payload: &[u8]) -> Result<Vec<u8>, StorageError> {
    envelope_with(payload, checksum)
}

pub(crate) fn envelope_with(
    payload: &[u8],
    digest: fn(&[u8]) -> [u8; 32],
) -> Result<Vec<u8>, StorageError> {
    if payload.is_empty() || payload.len() > limits::STORED_VALUE_MAX {
        return Err(StorageError::LimitExceeded {
            limit: "StoredValue",
            actual: payload.len(),
            allowed: limits::STORED_VALUE_MAX,
        });
    }
    let mut out = Vec::with_capacity(8 + payload.len() + limits::DIGEST_LEN);
    out.extend_from_slice(&ENVELOPE_MAGIC);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&digest(payload));
    Ok(out)
}
