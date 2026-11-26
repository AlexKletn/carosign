//! Trusted registration authorization with a separate confirmation call.
//!
//! The data form does not confirm trusted origin. The port implementation is obliged to check
//! the session with the broker and link the permission to the protected recipient.

mod authorized_enrollment;
mod challenge_observation;
mod enrollment_authorization_authority;
mod enrollment_authorization_error;
mod enrollment_authorization_scope;
mod enrollment_plan_binding;
mod enrollment_plan_change;
mod enrollment_recipient_id;
mod unavailable_enrollment_authority;
mod validated_challenge;

pub use authorized_enrollment::AuthorizedEnrollment;
pub use challenge_observation::ChallengeObservation;
pub use enrollment_authorization_authority::EnrollmentAuthorizationAuthority;
pub use enrollment_authorization_error::EnrollmentAuthorizationError;
pub use enrollment_authorization_scope::EnrollmentAuthorizationScope;
pub use enrollment_plan_binding::EnrollmentPlanBinding;
pub use enrollment_plan_change::EnrollmentPlanChange;
pub use enrollment_recipient_id::EnrollmentRecipientId;
pub use unavailable_enrollment_authority::UnavailableEnrollmentAuthority;
pub use validated_challenge::ValidatedChallenge;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
