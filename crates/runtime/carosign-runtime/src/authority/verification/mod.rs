//! Transfer of unverified intent to trusted sources.

mod invalid_intent;
mod purpose_hint;
mod requested_subject;
mod verification_authority_error;
mod verification_intent;

pub use invalid_intent::InvalidIntent;
pub use purpose_hint::PurposeHint;
pub use requested_subject::RequestedSubject;
pub use verification_authority_error::VerificationAuthorityError;
pub use verification_intent::VerificationIntent;
pub use verification_intent::authorize_verification;
