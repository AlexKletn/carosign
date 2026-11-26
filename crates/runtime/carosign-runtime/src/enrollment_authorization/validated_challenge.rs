use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::ids::{AuthorityId, GrantId};
use carosign_core::operation::context::BootInstant;

use super::{ChallengeObservation, EnrollmentAuthorizationError, EnrollmentAuthorizationScope};

/// Confirmed by a trusted port, not cloned.
pub struct ValidatedChallenge {
    scope: EnrollmentAuthorizationScope,
    grant_id: GrantId,
    grant_issuer: AuthorityId,
    challenge_ref: [u8; 16],
    digest: [u8; 32],
    issued_at: BootInstant,
    deadline: BootInstant,
}

fn check_grant(
    scope: &EnrollmentAuthorizationScope,
    grant: &ValidatedGrant,
    now: BootInstant,
) -> Result<(), EnrollmentAuthorizationError> {
    match grant.scope() {
        GrantScope::Enroll {
            subject,
            authenticator,
            ..
        } if *subject == scope.subject && *authenticator == scope.authenticator.id => {}
        _ => return Err(EnrollmentAuthorizationError::Invalid),
    }
    if now.boot() != scope.boot || grant.boot() != scope.boot {
        return Err(EnrollmentAuthorizationError::Invalid);
    }
    if now.instant_ns() >= grant.deadline_ns() {
        return Err(EnrollmentAuthorizationError::Expired);
    }
    Ok(())
}

impl ValidatedChallenge {
    /// Only for the trusted port after checking the session with the broker.
    /// The designer checks the shape, but does not prove a trusted origin.
    pub fn issued_for_port(
        scope: EnrollmentAuthorizationScope,
        observation: &ChallengeObservation,
        grant: &ValidatedGrant,
        now: BootInstant,
        deadline: BootInstant,
    ) -> Result<Self, EnrollmentAuthorizationError> {
        check_grant(&scope, grant, now)?;
        let limit = now
            .instant_ns()
            .checked_add(
                observation
                    .validity_ms()
                    .checked_mul(1_000_000)
                    .ok_or(EnrollmentAuthorizationError::Invalid)?,
            )
            .ok_or(EnrollmentAuthorizationError::Invalid)?;
        if deadline.boot() != scope.boot
            || deadline.instant_ns() <= now.instant_ns()
            || deadline.instant_ns() > limit
            || deadline.instant_ns() > grant.deadline_ns()
        {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(Self {
            scope,
            grant_id: grant.id(),
            grant_issuer: grant.issuer(),
            challenge_ref: *observation.challenge_ref(),
            digest: observation.digest(),
            issued_at: now,
            deadline,
        })
    }

    pub(super) fn restrict_deadline(
        &self,
        deadline: BootInstant,
    ) -> Result<Self, EnrollmentAuthorizationError> {
        if deadline.boot() != self.scope.boot
            || deadline.instant_ns() <= self.issued_at.instant_ns()
            || deadline.instant_ns() > self.deadline.instant_ns()
        {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(Self {
            scope: self.scope,
            grant_id: self.grant_id,
            grant_issuer: self.grant_issuer,
            challenge_ref: self.challenge_ref,
            digest: self.digest,
            issued_at: self.issued_at,
            deadline,
        })
    }

    /// Verifies the immutable area and term; the trusted port additionally checks the live
    /// issue, recall and secure session.
    pub fn check(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
        now: BootInstant,
    ) -> Result<(), EnrollmentAuthorizationError> {
        check_grant(scope, grant, now)?;
        if self.scope != *scope
            || self.grant_id != grant.id()
            || self.grant_issuer != grant.issuer()
        {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        if now.instant_ns() < self.issued_at.instant_ns() {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        if now.instant_ns() >= self.deadline.instant_ns() {
            return Err(EnrollmentAuthorizationError::Expired);
        }
        Ok(())
    }
    pub fn scope(&self) -> &EnrollmentAuthorizationScope {
        &self.scope
    }
    pub fn challenge_ref(&self) -> &[u8; 16] {
        &self.challenge_ref
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn deadline(&self) -> BootInstant {
        self.deadline
    }

    /// It does not allow you to accept confirmation from another surveillance provider.
    pub fn check_observation(
        &self,
        observation: &ChallengeObservation,
    ) -> Result<(), EnrollmentAuthorizationError> {
        if self.challenge_ref != *observation.challenge_ref() || self.digest != observation.digest()
        {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(())
    }
}
