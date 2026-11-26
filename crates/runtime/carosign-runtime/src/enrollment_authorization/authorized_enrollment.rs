use carosign_core::authorization::ValidatedGrant;
use carosign_core::operation::context::BootInstant;

use super::{EnrollmentAuthorizationError, EnrollmentAuthorizationScope, ValidatedChallenge};

/// Limited reference; not an independent authority.
/// The port and the protected recipient are required to check it every time they use it.
pub struct AuthorizedEnrollment {
    challenge: ValidatedChallenge,
    reference: [u8; 16],
}

impl AuthorizedEnrollment {
    /// For a trusted source after the confirmation of the user and the recipient.
    pub fn issued_for_port(
        challenge: &ValidatedChallenge,
        reference: [u8; 16],
    ) -> Result<Self, EnrollmentAuthorizationError> {
        Self::issued_for_port_until(challenge, reference, challenge.deadline())
    }

    /// The source may shorten the period of permission regarding the term of the call.
    pub fn issued_for_port_until(
        challenge: &ValidatedChallenge,
        reference: [u8; 16],
        deadline: BootInstant,
    ) -> Result<Self, EnrollmentAuthorizationError> {
        if reference == [0; 16] {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(Self {
            challenge: challenge.restrict_deadline(deadline)?,
            reference,
        })
    }

    /// Checks the immutability of the scope, permission and term of reference.
    pub fn check(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
        now: BootInstant,
    ) -> Result<(), EnrollmentAuthorizationError> {
        self.challenge.check(scope, grant, now)
    }
    pub fn reference(&self) -> &[u8; 16] {
        &self.reference
    }
    pub fn digest(&self) -> &[u8; 32] {
        self.challenge.digest()
    }
    pub fn challenge(&self) -> &ValidatedChallenge {
        &self.challenge
    }
}
