//! Canonical Bytes of Keys and Values (Document 23)
//!
//! ⚠️ The keys are bytes **glue**, no separators and no length: the lengths are known from
//! the scheme, and adding a separator would break the reconciliation with the document.
//!
//! ⚠️ Meaning is shell. `BIO1` + length + load + checksum.
//! The checksum here catches a random spoil. It doesn't protect you from someone who can
//! rewrite the base: the rollback and swap protection comes from a trusted support,
//! separately.

use core::fmt;

use crate::limits;
use crate::model::{MutationRecord, StorageError};

/// The magic of the sheath of meaning.
pub const ENVELOPE_MAGIC: [u8; 4] = *b"BIO1";

/// Supported version of the circuit.
pub const SCHEMA_VERSION: u32 = 1;

/// Key to the table `enrollments` / `mutations` / `authenticators` - ID.
///
/// ⚠️ The identifier is transmitted as a canonical 16 bytes: these tables do not have any
/// other type of key.
pub fn id_key(id: &[u8; limits::ID_KEY_LEN]) -> [u8; limits::ID_KEY_LEN] {
    *id
}

/// Key area `mutation_scope_lock`: subject + apparatus, 48 bytes.
pub fn scope_lock_key(record: &MutationRecord) -> [u8; limits::SCOPE_LOCK_KEY_LEN] {
    record.scope_key()
}

/// Key key `enrollment_index`Subject + apparatus + registration, 64 bytes.
///
/// ⚠️ The key is derived from the ** same ** entry as the value: the discrepancy would turn
/// "no in the index" into "no registration."
pub fn enrollment_index_key(
    record: &crate::enrollment_codec::EnrollmentRecord,
) -> [u8; limits::ENROLLMENT_INDEX_KEY_LEN] {
    let mut key = [0u8; limits::ENROLLMENT_INDEX_KEY_LEN];
    key[..limits::SUBJECT_CANONICAL_LEN].copy_from_slice(&record.subject.to_canonical());
    key[limits::SUBJECT_CANONICAL_LEN..limits::SCOPE_LOCK_KEY_LEN]
        .copy_from_slice(record.enrollment.authenticator.as_bytes());
    key[limits::SCOPE_LOCK_KEY_LEN..].copy_from_slice(record.enrollment.id.as_bytes());
    key
}

/// Key key `effect_outbox`: operation + transition + position.
pub fn outbox_key(
    operation: &[u8; limits::ID_KEY_LEN],
    transition: u64,
    position: u32,
) -> [u8; limits::OUTBOX_KEY_LEN] {
    let mut key = [0u8; limits::OUTBOX_KEY_LEN];
    key[..16].copy_from_slice(operation);
    key[16..24].copy_from_slice(&transition.to_be_bytes());
    key[24..].copy_from_slice(&position.to_be_bytes());
    key
}

/// Key to idempotence: Digesting the caller and the client key.
pub fn idempotency_key(
    caller_digest: &[u8; limits::DIGEST_LEN],
    key: &carosign_core::ids::IdempotencyKey,
) -> [u8; limits::IDEMPOTENCY_KEY_LEN] {
    let mut out = [0; limits::IDEMPOTENCY_KEY_LEN];
    out[..limits::DIGEST_LEN].copy_from_slice(caller_digest);
    out[limits::DIGEST_LEN..].copy_from_slice(key.as_bytes());
    out
}

/// Key key `template_blobs`: registration + generation, 24 bytes.
///
/// ⚠️ This key is the **sole* owner of the pattern identity, and the mirrors in meaning are
/// tested against it, not the other way around (solution R-01).
pub fn template_key(
    enrollment: &[u8; limits::ID_KEY_LEN],
    generation: u64,
) -> [u8; limits::TEMPLATE_KEY_LEN] {
    let mut key = [0u8; limits::TEMPLATE_KEY_LEN];
    key[..16].copy_from_slice(enrollment);
    key[16..].copy_from_slice(&generation.to_be_bytes());
    key
}

/// The sheath of meaning.
pub struct Envelope {
    /// A version of the circuit.
    pub schema: u32,
    /// Load.
    pub payload: Vec<u8>,
}

impl fmt::Debug for Envelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope")
            .field("schema", &self.schema)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

impl Envelope {
    /// He's collecting a recording envelope.
    ///
    /// ⚠️ The checksum is considered **only over the load** - including the length in the sum
    /// would check the same twice.
    pub fn encode(&self) -> Result<Vec<u8>, StorageError> {
        if self.schema != SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema { found: self.schema });
        }
        if self.payload.is_empty() || self.payload.len() > limits::STORED_VALUE_MAX {
            return Err(StorageError::LimitExceeded {
                limit: "StoredValue",
                actual: self.payload.len(),
                allowed: limits::STORED_VALUE_MAX,
            });
        }
        let mut out = Vec::with_capacity(8 + self.payload.len() + limits::DIGEST_LEN);
        out.extend_from_slice(&ENVELOPE_MAGIC);
        out.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.payload);
        out.extend_from_slice(&checksum(&self.payload));
        Ok(out)
    }

    /// Takes apart the recorded value, checking the **everything.
    pub fn decode(bytes: &[u8]) -> Result<Self, StorageError> {
        if bytes.len() < 8 + limits::DIGEST_LEN {
            return Err(StorageError::Corrupt {
                what: "оболочка короче заголовка",
            });
        }
        if bytes[..4] != ENVELOPE_MAGIC {
            return Err(StorageError::Corrupt {
                what: "магия оболочки",
            });
        }
        let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        if len == 0 || len > limits::STORED_VALUE_MAX {
            return Err(StorageError::LimitExceeded {
                limit: "StoredValue",
                actual: len,
                allowed: limits::STORED_VALUE_MAX,
            });
        }
        if bytes.len() != 8 + len + limits::DIGEST_LEN {
            return Err(StorageError::Corrupt {
                what: "длина оболочки",
            });
        }
        let payload = &bytes[8..8 + len];
        let digest = &bytes[8 + len..];
        if digest != checksum(payload) {
            return Err(StorageError::Corrupt {
                what: "контрольная сумма",
            });
        }
        Ok(Self {
            schema: SCHEMA_VERSION,
            payload: payload.to_vec(),
        })
    }
}

/// Load checksum.
///
/// ⚠️ The temporary checksum does not match SHA-256 of document 23.
/// Replacing it affects existing data and requires a separate migration.
pub fn checksum(payload: &[u8]) -> [u8; limits::DIGEST_LEN] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in payload {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut out = [0u8; limits::DIGEST_LEN];
    for (i, chunk) in out.chunks_mut(8).enumerate() {
        let v = h
            .wrapping_mul((i as u64) | 1)
            .wrapping_add(0x9e37_79b9_7f4a_7c15);
        chunk.copy_from_slice(&v.to_le_bytes());
    }
    out
}

#[path = "codec.tests.rs"]
#[cfg(test)]
#[allow(clippy::needless_borrow)]
mod tests;
