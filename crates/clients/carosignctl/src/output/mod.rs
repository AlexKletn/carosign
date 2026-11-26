//! Conclusion for the person: without secrets and without patterns.
//!
//! ⚠️ Rule of Document 07: Public output contains no template bytes, no token, no image. It's
//! only labels, states, and numbers.

mod authenticator_line;
mod enrollment_line;
mod short;

pub use authenticator_line::AuthenticatorLine;
pub use enrollment_line::EnrollmentLine;
pub use short::short;
