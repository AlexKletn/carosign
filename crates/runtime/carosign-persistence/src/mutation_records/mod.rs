//! Encoding change records and mutation log operations.
//!
//! The codec itself does not open the database, does not migrate data, and does not allow
//! sending.
//! Observation digest is not evidence of a physical condition.

mod arm;
#[cfg(test)]
mod arm_attempt_owner;
mod carosign;
mod codec;
mod completion;
mod durable_arm;
mod durable_arm_failure;
mod fresh_preparation_failure;
mod fresh_prepared_arm;
mod no_change;
mod observation;
pub(crate) use observation::stage_recovery_observation;
mod ownership;
mod preparation;
mod projection;
mod reconciliation;
mod record_error;
mod registry_update;

pub use arm::stage_arm;
pub use codec::{
    decode_idempotency, decode_mutation, decode_outbox, encode_idempotency, encode_mutation,
    encode_outbox, validate_mutation_outbox,
};
pub use completion::stage_completed;
pub use durable_arm_failure::DurableArmFailure;
pub use fresh_preparation_failure::FreshPreparationFailure;
pub use no_change::stage_pre_dispatch_no_change;
pub(crate) use observation::stage_enrollment_rollback;
pub use observation::{ObservedMutation, stage_observation};
pub use preparation::stage_prepared;
pub use projection::enrollment_projection;
pub(crate) use reconciliation::stage_needs_reconciliation;
pub use registry_update::{RegistryUpdate, stage_registry_update};

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
