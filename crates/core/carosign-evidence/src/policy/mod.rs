//! Factor Acceptance Policy: What is considered a success and why?
//!
//! ⚠️ `AcceptedFactor` It's ** "accepted for this request"**, not "accepted for this request"
//! (document 07).
//!
//! ⚠️ The software warranty is suitable for local input by setting and **is not suitable**
//! for key operation under any configuration.

mod factor_decision;
mod trust_profile;

pub use factor_decision::{FactorDecision, decide};
pub use trust_profile::TrustProfile;
