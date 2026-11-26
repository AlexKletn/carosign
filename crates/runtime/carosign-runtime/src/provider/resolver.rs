//! Comparison of profiles with inventory without running someone else's code.
//!
//! ⚠️ There's not a single call from the provider: the rules are compared to what the
//! inventory already knows. Active trial is a separate solution with a separate resolution
//! (Document 09).

pub use crate::provider::packages::Resolution;
pub use crate::provider::packages::resolve;
