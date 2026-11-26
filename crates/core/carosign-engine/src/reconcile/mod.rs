//! The result of an independent reconciliation after the cliff.
//!
//! ⚠️ "Registration is" or "it is not" in itself proves nothing.
//! Proves comparison with **expected ** initial and target state: no new registration
//! `Enroll(replace)` This does not mean that nothing has changed, but that the old one can be
//! removed (decision B-01).
//!
//! So the bottom line is ...`ReconciliationVerdict`] `bool`: either confirmed **desirable **,
//! or confirmed **original **, or nothing confirmed.
//!
//! ⚠️ Any successful outcome will pass through `Committing`: Even "nothing has changed" you
//! need to fix, close the log and remove the lock.

mod evaluate;
mod expected_states;
mod reconciliation_failure;
mod reconciliation_verdict;
mod registry_action;
mod verified_state;

pub use evaluate::evaluate;
pub use expected_states::ExpectedStates;
pub use reconciliation_failure::ReconciliationFailure;
pub use reconciliation_verdict::ReconciliationVerdict;
pub use registry_action::RegistryAction;
pub use verified_state::VerifiedState;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
