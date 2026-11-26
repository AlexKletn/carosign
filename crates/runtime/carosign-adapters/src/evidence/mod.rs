//! The proof checker is like a runtime port.
//!
//! ⚠️ The rule for which an adapter exists is: ** an worker's approval does not become an
//! assessment.** The port leads to the examiner, and he alone delivers the class.The
//! unavailable examiner is a refusal (Document 11).

mod evidence_bridge;
mod evidence_check;
mod protected_answer;
mod unavailable;

pub use evidence_bridge::EvidenceBridge;
pub use evidence_bridge::refusal_of;
pub use evidence_check::EvidenceCheck;
pub use protected_answer::ProtectedAnswer;
pub use unavailable::Unavailable;
