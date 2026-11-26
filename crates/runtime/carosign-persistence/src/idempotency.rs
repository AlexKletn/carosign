//! Idempotence within the transaction preparation of the operation.
//!
//! The caller is obliged to receive a digest from the trusted request normalizer and fix the
//! new entry along with the log and block the mutation.

use crate::model::{IdempotencyRecord, StorageError};
use crate::store::{Table, WriteTransaction};

/// Verifies an existing record or prepares a new one without completing a transaction.
///
/// The result `Staged` If a failure or ambiguous fixation results, the caller must re-read
/// the database.
/// `Existing` It does not give the right to observe the operation; it is checked by the
/// source of the policy.
pub fn stage(
    tx: &mut dyn WriteTransaction,
    candidate: &IdempotencyRecord,
) -> Result<Reservation, ReservationError> {
    if let Some(existing) = lookup(tx, candidate)? {
        return Ok(Reservation::Existing(existing));
    }
    let key = crate::redb::codec::idempotency_key(&candidate.caller_digest, &candidate.key);
    let bytes = crate::mutation_records::encode_idempotency(candidate)
        .map_err(ReservationError::Storage)?;
    tx.put(Table::Idempotency, &key, &bytes)
        .map_err(ReservationError::Storage)?;
    Ok(Reservation::Staged)
}

pub(crate) fn lookup(
    tx: &dyn WriteTransaction,
    candidate: &IdempotencyRecord,
) -> Result<Option<IdempotencyRecord>, ReservationError> {
    let key = crate::redb::codec::idempotency_key(&candidate.caller_digest, &candidate.key);
    if let Some(bytes) = tx
        .get(Table::Idempotency, &key)
        .map_err(ReservationError::Storage)?
    {
        let existing = crate::mutation_records::decode_idempotency(&key, &bytes)
            .map_err(ReservationError::Storage)?;
        if existing.request_digest != candidate.request_digest {
            return Err(ReservationError::Conflict);
        }
        if existing.mutation.is_some() != candidate.mutation.is_some() {
            return Err(ReservationError::Storage(StorageError::Corrupt {
                what: "вид идемпотентной операции не соответствует записи",
            }));
        }
        return Ok(Some(existing));
    }
    Ok(None)
}

#[path = "idempotency.tests.rs"]
#[cfg(test)]
mod tests;

mod reservation;
pub use reservation::Reservation;
mod reservation_error;
pub use reservation_error::ReservationError;
