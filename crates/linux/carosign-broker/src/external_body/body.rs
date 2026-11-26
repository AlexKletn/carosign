use super::BodyError;

/// Large body limit (document 32): 8 MiB.
pub const BODY_MAX: usize = 8 * 1024 * 1024;

/// The limit of the normal load of the frame.
pub const PAYLOAD_MAX: usize = 32 * 1024;

/// Description of a large body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExternalBody {
    /// The offset in the file. The baseline is only 0.
    pub offset: u64,
    /// Length: 1.8 MiB.
    pub length: u64,
    /// SHA-256 bytes of this site.
    pub sha256: [u8; 32],
    /// The file size as the sender sees it.
    pub file_size: u64,
}

impl ExternalBody {
    /// Checks the description before reading bytes.
    ///
    /// ⚠️ Order: offset → length → file size. The hash is checked separately when the bytes
    /// have already been read.
    pub fn validate(&self) -> Result<(), BodyError> {
        if self.offset != 0 {
            return Err(BodyError::OffsetNotSupported {
                offset: self.offset,
            });
        }
        if self.length == 0 || self.length > BODY_MAX as u64 {
            return Err(BodyError::BadLength {
                length: self.length,
            });
        }
        if self.file_size != self.length {
            return Err(BodyError::SizeMismatch {
                length: self.length,
                file_size: self.file_size,
            });
        }
        Ok(())
    }

    /// Checks the contents against the hash.
    pub fn check_bytes(&self, bytes: &[u8]) -> Result<(), BodyError> {
        if bytes.len() != self.length as usize {
            return Err(BodyError::SizeMismatch {
                length: self.length,
                file_size: bytes.len() as u64,
            });
        }
        let digest = carosign_persistence::mutation_plan::sha256(bytes);
        if digest != self.sha256 {
            return Err(BodyError::DigestMismatch);
        }
        Ok(())
    }

    /// Gathering a body description.
    pub fn for_bytes(bytes: &[u8]) -> Result<Self, BodyError> {
        if bytes.is_empty() || bytes.len() > BODY_MAX {
            return Err(BodyError::BadLength {
                length: bytes.len() as u64,
            });
        }
        Ok(Self {
            offset: 0,
            length: bytes.len() as u64,
            sha256: carosign_persistence::mutation_plan::sha256(bytes),
            file_size: bytes.len() as u64,
        })
    }
}

/// Checks the limit of conventional arguments and the description of a single large body.
///
/// ⚠️ `payload` It can contain typed arguments simultaneously with `external_body`The
/// prohibition of duplication of raw body checks the typed handler: one length `payload` You
/// cannot distinguish its contents.
pub fn check_exclusive(payload_len: usize, body: Option<ExternalBody>) -> Result<(), BodyError> {
    if payload_len > PAYLOAD_MAX {
        return Err(BodyError::BadLength {
            length: payload_len as u64,
        });
    }
    match body {
        None => Ok(()),
        Some(body) => body.validate(),
    }
}

/// Checks the number of descriptors in the presence of a body.
pub fn check_fd_count(body: Option<ExternalBody>, received: usize) -> Result<(), BodyError> {
    match body {
        None if received == 0 => Ok(()),
        None => Err(BodyError::WrongFdCount { actual: received }),
        Some(_) if received == 1 => Ok(()),
        Some(_) => Err(BodyError::WrongFdCount { actual: received }),
    }
}

#[path = "body.tests.rs"]
#[cfg(test)]
mod tests;
