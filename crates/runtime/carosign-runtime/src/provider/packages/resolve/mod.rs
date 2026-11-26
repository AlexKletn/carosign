//! Candidate choice: clear choice of admin, single, priority or refusal.
//!
//! ⚠️ Rule 12: First, explicit admin binding, then **single* appropriate, then configured
//! priority. `Ambiguous`, rather than "first in the alphabet": choosing the alphabet will one
//! day tie registrations to someone else's finger.

mod candidate;
mod resolution;

pub use candidate::Candidate;
pub use resolution::Resolution;
pub use resolution::resolve;
