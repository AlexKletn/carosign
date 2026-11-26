//! Powers and Persons: A Policy File Under a Trusted Root.
//!
//! ⚠️ Solution B-06: The right is not taken from a manifesto or a request, but from a policy
//! file owned by the root.`/etc/carosign/policy.d`).
//! `polkit` Requiring it on a tablet is an unnecessary addiction; it is the source, not the
//! service, that matters.
//!
//! ⚠️ The format reads with its own code: an unfamiliar field is a refusal, as in the
//! manifesto, otherwise a typo will quietly expand the rights.
//!
//! ⚠️ We don't make up personalities. `IdentityAuthority` allows the subject according to the
//! rules of the same file (coincidence of uid with the declared), and not by text.

mod action;
mod file_identity;
mod file_policy;
mod policy_error;
mod policy_rules;
mod rule;
mod subject_encoding;

pub use action::Action;
pub use file_identity::FileIdentity;
pub use file_policy::FilePolicy;
pub use policy_error::PolicyError;
pub use policy_rules::Policy;
pub use policy_rules::parse;
pub use rule::Rule;
pub use subject_encoding::subject_of;
