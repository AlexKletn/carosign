//! Recovery at Startup: What to Do with Unfinished Changes
//!
//! ⚠️ A rule from document 10, repeated here as code:
//!
//! * before `Prepared` - nothing started;
//! * `Prepared` without `DispatchArmed` - nothing has been confirmed;
//! * s `DispatchArmed` And then the hardware could change, you need a reconciliation.
//! * Repeating the changing command after restarting is prohibited in any case.
//!
//! ⚠️ This module doesn't fix anything; it only tells you what to look
//! for.`carosign-runtime`).

use core::fmt;

use carosign_core::ids::{MutationId, OperationId};

use crate::model::{MutationPhase, MutationRecord, StorageError};
use crate::store::MetadataStore;

mod mutation_snapshot;
pub use mutation_snapshot::MutationSnapshot;
mod recovery_decision;
pub use recovery_decision::RecoveryDecision;
mod startup_scan;
pub use startup_scan::StartupScan;
mod blocked_mutation_scope;
pub use blocked_mutation_scope::BlockedMutationScope;
mod startup_scope_scan;
pub use startup_scope_scan::StartupScopeScan;

mod reconciliation_plan;
pub use reconciliation_plan::read_reconciliation_plan_v3;
mod completed_outcome;
pub(crate) use completed_outcome::completed_outcome_in_snapshot;
pub use completed_outcome::read_completed_by_operation_v3;
pub use completed_outcome::read_completed_mutation_v3;
mod registry_checks;
use registry_checks::check_idempotency_for_mutation;
use registry_checks::check_registry_v3;
mod read_mutation_snapshot;
pub use read_mutation_snapshot::read_mutation_snapshot_v3;
pub(crate) use read_mutation_snapshot::read_mutation_snapshot_v3_in_snapshot;
mod recovery_policy;
pub use recovery_policy::decide_for;
pub use recovery_policy::may_replay;
pub use recovery_policy::scopes_to_block;
