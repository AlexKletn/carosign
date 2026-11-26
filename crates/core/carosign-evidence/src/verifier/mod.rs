//! Verifier: Why the evidence is accepted or not.
//!
//! ⚠️ `Unknown` The inaccessible verifier doesn't lower the requirements: he refuses.
//!
//! ⚠️ Verification is divided into **verifiable** (which we can establish ourselves) and
//! **guided** (which the secure module does). You can't mix them: then "verified" will mean
//! "presumed."

mod evidence_context;
mod observed;
mod protected_answer;
mod verdict;
mod verify;

pub use evidence_context::EvidenceContext;
pub use observed::Observed;
pub use protected_answer::ProtectedAnswer;
pub use verdict::Verdict;
pub use verify::verify;
