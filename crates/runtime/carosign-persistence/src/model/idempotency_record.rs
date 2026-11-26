use core::fmt;

use carosign_core::ids::{IdempotencyKey, MutationId, OperationId};

use crate::limits;

/// A record of idempotence.
#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecord {
    /// The hash of the caller's tether.
    pub caller_digest: [u8; limits::DIGEST_LEN],
    /// Client key.
    pub key: IdempotencyKey,
    /// A hash of normalized request.
    pub request_digest: [u8; limits::DIGEST_LEN],
    /// The surgery you chose.
    pub operation: OperationId,
    /// A change, if it is a change.
    pub mutation: Option<MutationId>,
}

impl fmt::Debug for IdempotencyRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdempotencyRecord(..)")
    }
}
