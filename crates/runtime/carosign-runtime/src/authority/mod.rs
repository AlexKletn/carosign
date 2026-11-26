//! Authorities: who calls, who is the subject, what is allowed, what is selected from this in
//! the set of registrations.
//!
//! ⚠️ Rule 08: No type here is going out of the wire.
//! `ValidatedGrant` It's the source of the policy. `SubjectRef` - the source of
//! personalities, and the set of authorized registrations - selection from the register.

pub mod caller;
pub mod grant;
pub mod revocation;
pub mod select;
pub mod subject;
pub mod verification;

pub use select::{Candidate, Custody, SelectionError, Status, select};
