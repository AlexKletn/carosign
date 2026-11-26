//! The policy of the mediator: what resource, what role and to whom.
//!
//! ⚠️ Rule 09: **Manifesto announces, policy issues.* So there is no reading of the
//! manifesto: the policy is set by the owner of the system (now by the build code, then by
//! the file under a trusted root).
//!
//! ⚠️ "There's no handler." `UNSUPPORTED`It's not a full-blown pass. `invoke_any_ioctl` It
//! doesn't exist by construction.

mod allowance;
mod broker_policy;
mod handler;
mod policy_error;

pub use allowance::Allowance;
pub use broker_policy::Policy;
pub use handler::Handler;
pub use policy_error::PolicyError;
