use carosign_core::authorization::ValidatedGrant;
use carosign_core::device::AuthenticatorRef;

use crate::ports::BoxFuture;

use super::{
    AuthorizedEnrollment, ChallengeObservation, EnrollmentAuthorizationError,
    EnrollmentAuthorizationScope, EnrollmentPlanBinding, EnrollmentRecipientId, ValidatedChallenge,
};

/// Trusted source of confirmation, without it, call registration is closed.
/// The implementation verifies the broker, recipient, live lease and disposability; no
/// secret material is returned to the caller.
pub trait EnrollmentAuthorizationAuthority: Send + Sync {
    fn bind_plan(
        &self,
        _scope: &EnrollmentAuthorizationScope,
        _grant: &ValidatedGrant,
        _plan: &EnrollmentPlanBinding,
    ) -> Result<(), EnrollmentAuthorizationError> {
        Err(EnrollmentAuthorizationError::Unavailable)
    }
    /// The Trusted Owner receives a call after training the provider.
    fn prepare_owner_challenge(
        &self,
        _scope: &EnrollmentAuthorizationScope,
        _grant: &ValidatedGrant,
        _send_boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> Result<ChallengeObservation, EnrollmentAuthorizationError> {
        Err(EnrollmentAuthorizationError::Unavailable)
    }
    /// Links opaque data to an already confirmed session.
    fn bind_owner(
        &self,
        _authorization: &AuthorizedEnrollment,
        _scope: &EnrollmentAuthorizationScope,
        _grant: &ValidatedGrant,
        _binding: Vec<u8>,
    ) -> Result<(), EnrollmentAuthorizationError> {
        Err(EnrollmentAuthorizationError::Unavailable)
    }
    /// Called under serialized admission; unknown result does not allow repeating the action.
    fn execute_owner(
        &self,
        _authorization: &AuthorizedEnrollment,
        _scope: &EnrollmentAuthorizationScope,
        _grant: &ValidatedGrant,
        _send_boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> crate::enrollment_admission::ConsumptionOutcome {
        crate::enrollment_admission::ConsumptionOutcome::Unknown
    }
    /// One-time opaque linking of the owner to a confirmed session; no credential or secrets;
    /// source verifies scope, grant and review; value alone does not permit action.
    fn take_owner_binding(
        &self,
        _authorization: &AuthorizedEnrollment,
        _scope: &EnrollmentAuthorizationScope,
        _grant: &ValidatedGrant,
    ) -> Result<Vec<u8>, EnrollmentAuthorizationError> {
        Err(EnrollmentAuthorizationError::Unavailable)
    }
    /// Finds a specific secure recipient from a trusted configuration.
    /// The worker’s input data is not the source of this reference.
    fn recipient_for(
        &self,
        _authenticator: AuthenticatorRef,
    ) -> Result<EnrollmentRecipientId, EnrollmentAuthorizationError> {
        Err(EnrollmentAuthorizationError::Unavailable)
    }
    /// Simultaneously revokes the session and one-time link and destroys or disables
    /// classified material from all trusted recipients.
    /// The method is suitable for future/Drop cancellation and must be idempotent. The error
    /// does not confirm the recall and requires quarantine; asynchronous `invalidate` It does
    /// not in itself replace this challenge.
    fn revoke_now(
        &self,
        _scope: &EnrollmentAuthorizationScope,
    ) -> Result<(), EnrollmentAuthorizationError> {
        Err(EnrollmentAuthorizationError::Unavailable)
    }
    /// Prior to sending, the challenge confirms the safe preparation and availability of a
    /// trusted broker and recipient.
    fn assess_preparation<'a>(
        &'a self,
        _scope: &'a EnrollmentAuthorizationScope,
        _grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<(), EnrollmentAuthorizationError>> {
        Box::pin(async { Err(EnrollmentAuthorizationError::Unavailable) })
    }
    /// Verifies surveillance by an independent record of a trusted broker.
    fn validate_session<'a>(
        &'a self,
        _scope: &'a EnrollmentAuthorizationScope,
        _observation: &'a ChallengeObservation,
        _grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<ValidatedChallenge, EnrollmentAuthorizationError>> {
        Box::pin(async { Err(EnrollmentAuthorizationError::Unavailable) })
    }
    /// Receives user confirmation for a limited recipient.
    fn authorize<'a>(
        &'a self,
        _challenge: &'a ValidatedChallenge,
        _grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<AuthorizedEnrollment, EnrollmentAuthorizationError>> {
        Box::pin(async { Err(EnrollmentAuthorizationError::Unavailable) })
    }
    /// Prior to arm, re-verifies the permit, deadline, issue and recall.
    fn revalidate<'a>(
        &'a self,
        _authorization: &'a AuthorizedEnrollment,
        _scope: &'a EnrollmentAuthorizationScope,
        _grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<(), EnrollmentAuthorizationError>> {
        Box::pin(async { Err(EnrollmentAuthorizationError::Unavailable) })
    }
    /// Revokes an incomplete session and destroys local links.
    fn invalidate<'a>(
        &'a self,
        _scope: &'a EnrollmentAuthorizationScope,
        _challenge: Option<ValidatedChallenge>,
        _authorization: Option<AuthorizedEnrollment>,
    ) -> BoxFuture<'a, Result<(), EnrollmentAuthorizationError>> {
        Box::pin(async { Err(EnrollmentAuthorizationError::Unavailable) })
    }
}
