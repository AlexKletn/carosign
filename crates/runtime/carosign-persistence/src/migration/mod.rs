//! Preparation of explicit metadata migration and closed switching stage.
//!
//! A conventional build does not give a switching tolerance: the backup, external lock and
//! old binary are not yet connected.

use redb::TableHandle;

use crate::installation_records::{MigratableRecord, prepare_v3_record};
use crate::store::{ReadTransaction, Table};

mod journal;
mod staging;

mod schema;
use schema::{JOURNAL_KEY, STAGING_TABLE};

// There's no real source of clearance yet. Fields are closed in a separate module, so you
// can't create that clearance with just a log entry or a matching ID.
mod admission;

#[path = "migration.tests.rs"]
#[cfg(test)]
mod tests;

mod switch_failure;
use switch_failure::SwitchFailure;
mod cleanup_failure;
pub(crate) use cleanup_failure::CleanupFailure;
mod preflight_failure;
pub(crate) use preflight_failure::PreflightFailure;
mod preflight_summary;
pub(crate) use preflight_summary::PreflightSummary;

mod backup_recovery;
#[cfg(test)]
use backup_recovery::inspect_recovery_backup;
mod publish_pending;
#[cfg(test)]
use publish_pending::publish_pending;
#[cfg(test)]
use publish_pending::publish_pending_inner;
mod stage_pending;
#[cfg(test)]
use stage_pending::stage_pending;
#[cfg(test)]
use stage_pending::stage_pending_inner;
mod switch_staged;
pub(crate) use switch_staged::is_committed_v4_journal;
#[cfg(test)]
use switch_staged::switch_staged;
#[cfg(test)]
use switch_staged::switch_staged_inner;
mod cleanup_staging;
#[allow(unused_imports)]
pub(crate) use cleanup_staging::cleanup_committed_staging;
mod staging_audit;
pub(crate) use staging_audit::audit_staging_snapshot;
mod snapshot_preflight;
pub(crate) use snapshot_preflight::preflight_v3_snapshot;
use snapshot_preflight::preflight_v3_snapshot_inner;
