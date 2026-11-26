//! An immutable plan of change is one without which recovery lies.
//!
//! ⚠️ Rule 33: Plan is written in the same** stable transaction as `MUTATION_PREPARED`Without
//! it, there's no answer to the question of "what was going to be," and recovery becomes a
//! guess.
//!
//! ⚠️ The plan contains the logical region, not the interior of the hardware.
//! There are no templates, frames and closed bytes in it.
//!
//! ⚠️ The digest here is **consistency control**, not trust support or rollback protection
//! (document 33 explicitly stipulates this).

mod codec;
mod digest;
mod sha256;
mod subject_of;

pub use codec::{decode_plan, encode_plan};
pub use digest::{canonical_digest, canonical_request_digest};
pub use sha256::sha256;
pub use subject_of::subject_of;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;

mod transition;
pub use transition::Transition;
mod stored_binding_kind;
pub use stored_binding_kind::StoredBindingKind;
mod binding_snapshot;
pub use binding_snapshot::BindingSnapshot;
mod mutation_plan;
pub use mutation_plan::{MutationPlan, SNAPSHOTS_MAX};
mod plan_error;
pub use plan_error::PlanError;
mod scope;
pub use scope::Scope;
mod plan_cursor;
use plan_cursor::PlanCursor;
