//! An immutable logical plan for changing the registration area.
//!
//! The plan describes the comparison, but does not prove the origin of the image or the
//! physical condition of the device.

mod binding_kind;
mod binding_snapshot;
mod mutation_plan;
mod plan_error;
mod plan_transition;

pub use binding_kind::BindingKind;
pub use binding_snapshot::BindingSnapshot;
pub use mutation_plan::MutationPlan;
pub use plan_error::PlanError;
pub use plan_transition::PlanTransition;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
