//! Domain errors and the limits that cause them.
//!
//! ⚠️ The domain responds with a **type**, not a line: "didn't match" and "assembly broken"
//! are different things (document 13). `Error(String)` It doesn't cross the boundary.
//!
//! The limits here are **domain numbers**, not time settings. Setup can only tighten them;
//! expand - a separate solution (Document 13).

mod domain_error;
mod limit_exceeded;
pub mod limits;

pub use domain_error::DomainError;
pub use limit_exceeded::LimitExceeded;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
