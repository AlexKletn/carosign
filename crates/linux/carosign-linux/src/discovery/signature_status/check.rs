use super::SignatureRefusal;

pub(super) fn fingerprint(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
}

/// Confirms exactly one correct signature of the approved master key.
/// Signing connections are allowed with the main key specified in VALIDSIG.
pub(in crate::discovery) fn check(
    success: bool,
    status: &[u8],
    approved_primary: &str,
) -> Result<(), SignatureRefusal> {
    if !success {
        return Err(SignatureRefusal::VerifierFailed);
    }
    if !fingerprint(approved_primary) {
        return Err(SignatureRefusal::InvalidFingerprint);
    }
    if status.len() > 65_536 {
        return Err(SignatureRefusal::InvalidStatus);
    }
    let text = std::str::from_utf8(status).map_err(|_| SignatureRefusal::InvalidStatus)?;
    let mut valid = None;
    for line in text.lines() {
        let line = line
            .strip_prefix("[GNUPG:] ")
            .ok_or(SignatureRefusal::InvalidStatus)?;
        let mut parts = line.split_ascii_whitespace();
        match parts.next().ok_or(SignatureRefusal::InvalidStatus)? {
            "VALIDSIG" => {
                let fields: Vec<_> = parts.collect();
                if valid.is_some()
                    || fields.len() < 9
                    || fields.len() > 10
                    || !fingerprint(fields[0])
                {
                    return Err(SignatureRefusal::InvalidStatus);
                }
                let primary = if fields.len() == 10 {
                    fields[9]
                } else {
                    fields[0]
                };
                if !fingerprint(primary) {
                    return Err(SignatureRefusal::InvalidStatus);
                }
                valid = Some(primary);
            }
            "BADSIG" | "ERRSIG" | "EXPSIG" | "EXPKEYSIG" | "REVKEYSIG" | "KEYEXPIRED"
            | "SIGEXPIRED" | "FAILURE" | "ERROR" | "NO_PUBKEY" | "NODATA" | "UNEXPECTED" => {
                return Err(SignatureRefusal::InvalidStatus);
            }
            _ => {}
        }
    }
    match valid {
        Some(value) if value == approved_primary => Ok(()),
        Some(_) => Err(SignatureRefusal::NotApproved),
        None => Err(SignatureRefusal::InvalidStatus),
    }
}
