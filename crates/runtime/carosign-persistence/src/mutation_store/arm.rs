use crate::journal::ArmMutation;
use crate::model::StorageError;
use crate::store::{Table, WriteTransaction};

use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Preparing a recording. DispatchArmed and the queue of the effect in one transaction v4.
///
/// It's not a confirmation of a fixation, it's not a right to send. It's the caller's duty to
/// verify the authority, the effect's ownership, and to interrupt the transaction if it's
/// wrong; after an unknown fixation outcome, it's forbidden to repeat.
pub fn stage_arm_v4(
    tx: &mut dyn WriteTransaction,
    input: &ArmMutation,
    effect: carosign_engine::PlannedEffect,
) -> Result<(), StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    crate::mutation_records::stage_arm(&mut RecordView { inner: tx }, input, effect)
}
