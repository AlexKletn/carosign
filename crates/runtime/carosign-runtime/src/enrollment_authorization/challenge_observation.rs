use sha2::{Digest, Sha256};

use super::EnrollmentAuthorizationError;

/// Unverified worker response data, the form does not confirm the session.
pub struct ChallengeObservation {
    challenge: Vec<u8>,
    challenge_ref: [u8; 16],
    validity_ms: u64,
}

impl ChallengeObservation {
    /// Takes a limited value; the origin checks the trusted port.
    pub fn new(
        challenge: Vec<u8>,
        challenge_ref: [u8; 16],
        validity_ms: u64,
        max_validity_ms: u64,
    ) -> Result<Self, EnrollmentAuthorizationError> {
        if !(1..=64).contains(&challenge.len())
            || challenge_ref == [0; 16]
            || validity_ms == 0
            || validity_ms > max_validity_ms
            || validity_ms.checked_mul(1_000_000).is_none()
        {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(Self {
            challenge,
            challenge_ref,
            validity_ms,
        })
    }

    pub fn challenge(&self) -> &[u8] {
        &self.challenge
    }
    pub fn challenge_ref(&self) -> &[u8; 16] {
        &self.challenge_ref
    }
    pub fn validity_ms(&self) -> u64 {
        self.validity_ms
    }
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(&self.challenge).into()
    }
}
