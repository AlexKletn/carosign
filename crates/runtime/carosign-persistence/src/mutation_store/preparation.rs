use crate::idempotency::{Reservation, ReservationError};
use crate::journal::PrepareMutation;
use crate::model::IdempotencyRecord;
use crate::store::{Table, WriteTransaction};

use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Prepares log, lock and idempotence in one v4 transaction.
///
/// `Staged` does not mean stable fixation and does not give the right to send.
/// The origin of the request, metadata and authority is verified by the owner.
pub fn stage_prepared_v4(
    tx: &mut dyn WriteTransaction,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
) -> Result<Reservation, ReservationError> {
    stage_prepared_with_schema(tx, input, idempotency, SCHEMA_V4)
}

/// Prepares a change only in the verified transaction of the primary v5 scheme.
/// Stable fixation and powers are checked separately.
#[allow(dead_code)]
pub(crate) fn stage_prepared_v5(
    tx: &mut crate::redb::backend::GuardedWriteTx<'_>,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
) -> Result<Reservation, ReservationError> {
    stage_prepared_with_schema(tx, input, idempotency, b"BSC1\0\0\0\x05\0\0\0\0")
}

fn stage_prepared_with_schema(
    tx: &mut dyn WriteTransaction,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
    schema: &[u8],
) -> Result<Reservation, ReservationError> {
    let meta = tx
        .scan_prefix(Table::Meta, &[])
        .map_err(ReservationError::Storage)?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != schema {
        return Err(ReservationError::Storage(corrupt()));
    }
    if let Some(label) = input.plan.requested_label.as_deref() {
        let scope = crate::mutation_plan::Scope {
            subject: input.subject,
            authenticator: input.authenticator,
        };
        let digest = crate::mutation_plan::canonical_request_digest(
            scope,
            input.plan.transition,
            input.plan.old_target,
            Some(label),
        )
        .map_err(|_| ReservationError::Storage(corrupt()))?;
        if digest != idempotency.request_digest {
            return Err(ReservationError::Storage(corrupt()));
        }
    }
    crate::mutation_records::stage_prepared(&mut RecordView { inner: tx }, input, idempotency)
}

#[cfg(test)]
#[path = "preparation.tests.rs"]
mod tests;
