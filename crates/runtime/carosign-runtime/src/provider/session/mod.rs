//! The session of the provider: what operation he is attached to and what he has already
//! said.
//!
//! The session tracks the accepted command and its outcome. It does not change the registry
//! or produce an outcome. A second outcome invalidates the session.

mod enrollment_outcome;
mod progress_observation;
mod scoped_provider_assertion;
mod session;
mod session_error;
mod verification_observation;

pub use enrollment_outcome::EnrollmentOutcome;
pub use progress_observation::ProgressObservation;
pub use scoped_provider_assertion::ScopedProviderAssertion;
pub use session::Session;
pub use session_error::SessionError;
pub use verification_observation::VerificationObservation;
