//! Preparation of a change in the scheme of the first installation v4.
//!
//! The public stages do not issue a dispatch ticket, nor do they allow repeat after
//! indefinite fixation.Internal possibility links fresh fixation to a one-time ticket, but
//! does not replace a trusted owner's check.
//! The owner of the transaction is obliged to interrupt it in case of an error.

mod arm;
mod completion;
mod dispatch_ticket;
mod durable_registry_failure;
mod enrollment;
mod fresh_prepared_arm;
mod no_change;
mod observation;
mod operation_status;
mod preparation;
mod projection;
mod read_view;
mod reconciliation;
mod reconciliation_plan;
mod record_view;
mod recover_armed;
mod recover_unarmed;
mod recovery_evidence;
mod registry_update;
mod rename_enrollment;
mod rollback;
mod schema;
mod snapshot;

pub use arm::stage_arm_v4;
pub use completion::{complete_v4, read_completed_v4, stage_completed_v4};
pub use dispatch_ticket::validate_dispatch_ticket_v4;
pub use durable_registry_failure::DurableRegistryFailure;
pub use enrollment::{decode_enrollment_in_snapshot, decode_enrollment_v4, encode_enrollment_v4};
pub use fresh_prepared_arm::{FreshPreparedArmV4, prepare_fresh_arm_v4};
pub use no_change::stage_pre_dispatch_no_change_v4;
#[allow(unused_imports)]
pub(crate) use no_change::stage_pre_dispatch_no_change_v5;
pub use observation::stage_observation_v4;
pub use operation_status::read_mutation_by_operation_v4;
pub use preparation::stage_prepared_v4;
#[allow(unused_imports)]
pub(crate) use preparation::stage_prepared_v5;
pub use projection::enrollment_projection_v4;
pub use reconciliation::stage_needs_reconciliation_v4;
pub use reconciliation_plan::read_reconciliation_plan_v4;
pub use recover_armed::{armed_recovery_targets_v4, recover_armed_v4};
pub use recover_unarmed::recover_unarmed_v4;
pub use recovery_evidence::RecoveryEvidence;
pub use registry_update::{commit_registry_v4, stage_registry_update_v4};
pub use rename_enrollment::rename_enrollment;
pub use rollback::commit_enrollment_rollback_v4;
pub(crate) use schema::SCHEMA_V4;
pub use snapshot::{inspect_prepared_v4, scan_prepared_v4_snapshot};
