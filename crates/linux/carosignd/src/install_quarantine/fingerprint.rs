use super::QuarantineFailure;

pub(super) fn decode_fingerprint(value: &str) -> Result<Vec<u8>, QuarantineFailure> {
    if !value.len().is_multiple_of(2) || !value.is_ascii() {
        return Err(QuarantineFailure::InvalidFingerprint);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |value: u8| match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'A'..=b'F' => Some(value - b'A' + 10),
                _ => None,
            };
            Ok(
                digit(pair[0]).ok_or(QuarantineFailure::InvalidFingerprint)? * 16
                    + digit(pair[1]).ok_or(QuarantineFailure::InvalidFingerprint)?,
            )
        })
        .collect()
}
