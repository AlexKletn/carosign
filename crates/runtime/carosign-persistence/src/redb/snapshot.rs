//! A photo of the magazine in memory for re-reading.
//!
//! ⚠️ Service type: the journal on the disk writes
//! [...]`StoreJournal`](crate::journal_store::StoreJournal)This image is only needed so that
//! recovery and reconciliation can keep a consistent set of records without opening the
//! transaction per record.
//!
//! ⚠️ The source of truth: when discordant with the disk rights disc.

use crate::model::MutationRecord;

/// A collection of journal entries read in one reading.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JournalSnapshot {
    records: Vec<MutationRecord>,
}

impl JournalSnapshot {
    /// Collects a picture from the read records.
    pub fn from_records(mut records: Vec<MutationRecord>) -> Self {
        records.sort_by_key(|record| record.mutation);
        Self { records }
    }

    /// Records are in order of numbers.
    pub fn records(&self) -> &[MutationRecord] {
        &self.records
    }

    /// The ones that aren't closed yet.
    pub fn incomplete(&self) -> Vec<&MutationRecord> {
        self.records
            .iter()
            .filter(|record| {
                crate::recovery::decide_for(record) != crate::recovery::RecoveryDecision::Closed
            })
            .collect()
    }

    /// The ones that might have touched the hardware.
    ///
    /// ⚠️ This is the list of areas to reconcile at launch.
    pub fn may_have_touched_hardware(&self) -> Vec<&MutationRecord> {
        self.records
            .iter()
            .filter(|record| record.phase.may_have_touched_hardware())
            .collect()
    }
}
