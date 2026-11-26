//! Worker events: progress and outcomes.
//!
//! ⚠️ An worker can't announce the success of authentication. `Matched` Proof: This is a
//! statement that the platform has yet to verify (Document 11).

mod event;
mod event_error;
mod provider_evidence;

pub use event::Event;
pub use event::MutationStatus;
pub use event::ProgressKind;
pub use event::VerificationStatus;
pub(crate) use event::check_error;
pub(crate) use event::check_mutation_outcome;
pub(crate) use event::check_query_result;
pub use event::parse;
pub use event_error::EventError;
pub use provider_evidence::ProviderEvidence;
