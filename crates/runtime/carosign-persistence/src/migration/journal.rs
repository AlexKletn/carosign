//! Closed codec of the accepted migration log v3 → v4.
//!
//! Codec doesn't back up, give us security clearance, or change the database.
#![allow(dead_code)]

use crate::mutation_plan::sha256;

const PAYLOAD_LEN: usize = 128;
const ENVELOPE_LEN: usize = 8 + PAYLOAD_LEN + 32;

#[path = "journal.tests.rs"]
#[cfg(test)]
mod tests;

/// Verifies the marker read from one image and the migration record.
pub(super) fn classify(schema: u32, bytes: Option<&[u8]>) -> Result<RecoveryState, InvalidJournal> {
    match (schema, bytes) {
        (3, None) => Ok(RecoveryState::Unstarted),
        (3, Some(bytes)) => match Journal::decode(bytes)?.stage {
            stage @ (Stage::Pending | Stage::Staged(_)) => {
                Ok(RecoveryState::AdministrativeRecovery(stage))
            }
            Stage::Committed(_) => Err(InvalidJournal),
        },
        (4, Some(bytes)) => match Journal::decode(bytes)?.stage {
            Stage::Committed(snapshot) => Ok(RecoveryState::Committed(snapshot)),
            Stage::Pending | Stage::Staged(_) => Err(InvalidJournal),
        },
        _ => Err(InvalidJournal),
    }
}

mod invalid_journal;
pub(super) use invalid_journal::InvalidJournal;
mod snapshot;
pub(super) use snapshot::Snapshot;
mod stage;
pub(super) use stage::Stage;
mod journal;
pub(super) use journal::Journal;
mod recovery_state;
pub(super) use recovery_state::RecoveryState;
