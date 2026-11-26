//! Selection of authorized registrations from the registry is what D-03 requires.
//!
//! ⚠️ Rule 29 (D-03): Verification is addressed to the **set of active authorized
//! registrations** of the "subject + authenticator" pair.
//!
//! ⚠️ There's no storage: it's clean. It takes the records that it's been given, and it takes
//! them out, and who reads it, and with what rights, is the defiant's business.

mod candidate;
mod custody;
mod selection_error;
mod status;

pub use candidate::Candidate;
pub use candidate::select;
pub use custody::Custody;
pub use selection_error::SelectionError;
pub use status::Status;
