//! Engine invariants are things that should never happen.
//!
//! ⚠️ The checks are on the exits of every transition, not in the documents.
//! This rule is easily broken inadvertently in the next order; let the check drop rather than
//! quietly get up to send to the hardware without writing to the disk.

mod check_effect_addresses;
mod check_phase;
mod check_reconciliation;
mod check_scope;
mod check_transition;
mod is_stale;

pub(crate) use check_effect_addresses::check_effect_addresses;
pub use check_phase::check_phase;
pub(crate) use check_reconciliation::check_reconciliation;
pub(crate) use check_scope::check_scope;
pub use check_transition::check;
pub use is_stale::is_stale;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
