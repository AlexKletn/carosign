//! One-time observation of changes to the Linux policy file.
//!
//! The observer only forbids the use of the old image; he himself does not confirm the origin
//! of the policy, the owner of the path or the authority.

mod policy_change_watch;
mod policy_watch_state;
mod protected_policy_snapshot;
mod system_trust_source;

pub use policy_change_watch::PolicyChangeWatch;
pub use policy_watch_state::PolicyWatchState;
pub use protected_policy_snapshot::ProtectedPolicySnapshot;
pub use system_trust_source::SystemTrustSource;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
