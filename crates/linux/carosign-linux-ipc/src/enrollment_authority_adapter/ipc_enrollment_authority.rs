use super::{Phase, Session, TrustedEnrollmentConnector};

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::device::AuthenticatorRef;
use carosign_core::operation::context::BootInstant;
use carosign_runtime::enrollment_authorization::{
    AuthorizedEnrollment, ChallengeObservation, EnrollmentAuthorizationAuthority,
    EnrollmentAuthorizationError as Error, EnrollmentAuthorizationScope, EnrollmentRecipientId,
    ValidatedChallenge,
};
use carosign_runtime::ports::{BoxFuture, TimeSource};

use crate::enrollment_authority::{self as wire, GrantBinding};

/// A disposable copy is created for one operation, and one object should not be reused for
/// different registrations or after the worker restarts.
pub struct IpcEnrollmentAuthority {
    connector: Arc<dyn TrustedEnrollmentConnector>,
    clock: Arc<dyn TimeSource>,
    session: Mutex<Option<Arc<Session>>>,
    operation_timeout: Duration,
    control_timeout: Duration,
}

impl IpcEnrollmentAuthority {
    pub fn new(
        connector: Arc<dyn TrustedEnrollmentConnector>,
        clock: Arc<dyn TimeSource>,
        operation_timeout: Duration,
        control_timeout: Duration,
    ) -> Result<Self, Error> {
        if operation_timeout.is_zero()
            || operation_timeout > Duration::from_secs(60)
            || control_timeout.is_zero()
            || control_timeout > Duration::from_secs(2)
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            connector,
            clock,
            session: Mutex::new(None),
            operation_timeout,
            control_timeout,
        })
    }

    fn check_grant(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
    ) -> Result<(), Error> {
        let now = self.clock.now();
        if !matches!(grant.scope(), GrantScope::Enroll { subject, authenticator, .. } if *subject == scope.subject && *authenticator == scope.authenticator.id)
            || grant.boot() != scope.boot
            || now.boot() != scope.boot
        {
            return Err(Error::Invalid);
        }
        if now.instant_ns() >= grant.deadline_ns() {
            return Err(Error::Expired);
        }
        Ok(())
    }

    fn session(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
    ) -> Result<Arc<Session>, Error> {
        self.check_grant(scope, grant)?;
        let session = self
            .session
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .ok_or(Error::Unavailable)?;
        if session.scope != *scope
            || session.grant != GrantBinding::from_grant(grant)
            || session.grant_scope != *grant.scope()
        {
            return Err(Error::Invalid);
        }
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        Ok(session)
    }

    fn send(
        &self,
        session: &Session,
        action: wire::request::Action,
    ) -> Result<wire::response::Result, Error> {
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        let mut channel = session.operation.lock().map_err(|_| Error::Unavailable)?;
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        let result = channel
            .request(session.grant.clone(), action, self.operation_timeout)
            .map_err(|_| Error::Unavailable)?;
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        if matches!(result, wire::response::Result::Refused(_)) {
            return Err(Error::Denied);
        }
        Ok(result)
    }
}

impl EnrollmentAuthorizationAuthority for IpcEnrollmentAuthority {
    fn bind_plan(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
        plan: &carosign_runtime::enrollment_authorization::EnrollmentPlanBinding,
    ) -> Result<(), Error> {
        use carosign_runtime::enrollment_authorization::EnrollmentPlanChange;
        if plan.planned.authenticator != scope.authenticator.id {
            return Err(Error::Invalid);
        }
        let (change, old_enrollment, old_generation, old_target) = match plan.change {
            EnrollmentPlanChange::Create => (wire::PlanChange::Create, Vec::new(), 0, None),
            EnrollmentPlanChange::Replace { old }
                if old.authenticator == scope.authenticator.id && old.id != plan.planned.id =>
            {
                (
                    wire::PlanChange::Replace,
                    old.id.as_bytes().to_vec(),
                    old.generation.value(),
                    Some(old),
                )
            }
            _ => return Err(Error::Invalid),
        };
        if !matches!(grant.scope(), GrantScope::Enroll { subject, authenticator, replace } if *subject == scope.subject && *authenticator == scope.authenticator.id && *replace == old_target)
        {
            return Err(Error::Invalid);
        }
        let session = self.session(scope, grant)?;
        let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
        let Phase::ChallengePrepared {
            challenge_ref,
            digest,
        } = *phase
        else {
            return Err(Error::Invalid);
        };
        *phase = Phase::Failed;
        let response = self.send(
            &session,
            wire::request::Action::BindPlan(wire::PlanBinding {
                planned_enrollment: plan.planned.id.as_bytes().to_vec(),
                generation: plan.planned.generation.value(),
                plan_digest: plan.digest.to_vec(),
                change: change as i32,
                old_enrollment,
                old_generation,
            }),
        )?;
        if !matches!(response, wire::response::Result::Accepted(_)) {
            return Err(Error::Unavailable);
        }
        *phase = Phase::PlanBound {
            challenge_ref,
            digest,
        };
        Ok(())
    }
    fn prepare_owner_challenge(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
        send_boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> Result<ChallengeObservation, Error> {
        let session = self.session(scope, grant)?;
        let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
        if *phase != Phase::Prepared {
            return Err(Error::Invalid);
        }
        *phase = Phase::Failed;
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        let mut operation = session.operation.lock().map_err(|_| Error::Unavailable)?;
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        let mut boundary = |send: &mut dyn FnMut() -> bool| {
            send_boundary(&mut || !session.revoked.load(Ordering::SeqCst) && send())
        };
        let response = operation
            .request_with_boundary(
                session.grant.clone(),
                wire::request::Action::PrepareChallenge(wire::Empty {}),
                self.operation_timeout,
                &mut boundary,
            )
            .map_err(|_| Error::Unavailable)?;
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        let wire::response::Result::Challenge(value) = response else {
            return Err(Error::Unavailable);
        };
        let observation = ChallengeObservation::new(
            value.challenge,
            value.challenge_ref.try_into().map_err(|_| Error::Invalid)?,
            value.validity_ms,
            60_000,
        )?;
        *phase = Phase::ChallengePrepared {
            challenge_ref: *observation.challenge_ref(),
            digest: observation.digest(),
        };
        Ok(observation)
    }

    fn bind_owner(
        &self,
        authorization: &AuthorizedEnrollment,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
        binding: Vec<u8>,
    ) -> Result<(), Error> {
        let session = self.session(scope, grant)?;
        authorization.check(scope, grant, self.clock.now())?;
        let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
        let expected = Phase::Authorized {
            authorization_ref: *authorization.reference(),
            digest: *authorization.digest(),
        };
        if *phase != expected || binding.is_empty() || binding.len() > 1024 {
            return Err(Error::Invalid);
        }
        *phase = Phase::Failed;
        if !matches!(
            self.send(
                &session,
                wire::request::Action::BindOwner(wire::OwnerBinding { payload: binding })
            )?,
            wire::response::Result::Accepted(_)
        ) {
            return Err(Error::Unavailable);
        }
        *phase = Phase::Bound {
            authorization_ref: *authorization.reference(),
            digest: *authorization.digest(),
        };
        Ok(())
    }

    fn execute_owner(
        &self,
        authorization: &AuthorizedEnrollment,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
        send_boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> carosign_runtime::enrollment_admission::ConsumptionOutcome {
        use carosign_runtime::enrollment_admission::ConsumptionOutcome;
        let Ok(session) = self.session(scope, grant) else {
            return ConsumptionOutcome::NotDispatched;
        };
        if authorization.check(scope, grant, self.clock.now()).is_err() {
            return ConsumptionOutcome::NotDispatched;
        }
        let Ok(mut phase) = session.phase.lock() else {
            return ConsumptionOutcome::Unknown;
        };
        if *phase
            != (Phase::Bound {
                authorization_ref: *authorization.reference(),
                digest: *authorization.digest(),
            })
        {
            return ConsumptionOutcome::NotDispatched;
        }
        *phase = Phase::Failed;
        if session.revoked.load(Ordering::SeqCst) {
            return ConsumptionOutcome::NotDispatched;
        }
        let Ok(mut operation) = session.operation.lock() else {
            return ConsumptionOutcome::Unknown;
        };
        if session.revoked.load(Ordering::SeqCst) {
            return ConsumptionOutcome::NotDispatched;
        }
        let mut boundary = |send: &mut dyn FnMut() -> bool| {
            send_boundary(&mut || !session.revoked.load(Ordering::SeqCst) && send())
        };
        let result = operation.request_with_boundary(
            session.grant.clone(),
            wire::request::Action::Execute(wire::Authorization {
                reference: authorization.reference().to_vec(),
                challenge_digest: authorization.digest().to_vec(),
            }),
            self.operation_timeout,
            &mut boundary,
        );
        if session.revoked.load(Ordering::SeqCst) {
            return ConsumptionOutcome::Unknown;
        }
        match result {
            Ok(wire::response::Result::Accepted(_)) => ConsumptionOutcome::Completed,
            _ => ConsumptionOutcome::Unknown,
        }
    }

    fn take_owner_binding(
        &self,
        authorization: &AuthorizedEnrollment,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
    ) -> Result<Vec<u8>, Error> {
        let session = self.session(scope, grant)?;
        authorization.check(scope, grant, self.clock.now())?;
        let phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
        if *phase
            != (Phase::Authorized {
                authorization_ref: *authorization.reference(),
                digest: *authorization.digest(),
            })
            || session.revoked.load(Ordering::SeqCst)
        {
            return Err(Error::Invalid);
        }
        let binding = session
            .owner_binding
            .lock()
            .map_err(|_| Error::Unavailable)?
            .take()
            .filter(|binding| !binding.is_empty())
            .ok_or(Error::Unavailable)?;
        if session.revoked.load(Ordering::SeqCst) {
            return Err(Error::Revoked);
        }
        Ok(binding)
    }

    fn recipient_for(
        &self,
        authenticator: AuthenticatorRef,
    ) -> Result<EnrollmentRecipientId, Error> {
        self.connector.recipient_for(authenticator)
    }

    fn assess_preparation<'a>(
        &'a self,
        scope: &'a EnrollmentAuthorizationScope,
        grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            self.check_grant(scope, grant)?;
            if self.connector.recipient_for(scope.authenticator)? != scope.recipient {
                return Err(Error::Invalid);
            }
            let (operation, control) = self.connector.connect(scope, grant)?;
            if !operation.belongs_to(scope) || !control.belongs_to(scope) {
                return Err(Error::Invalid);
            }
            let session = Arc::new(Session {
                scope: *scope,
                grant: GrantBinding::from_grant(grant),
                grant_scope: grant.scope().clone(),
                operation: Mutex::new(operation),
                control: Mutex::new(control),
                phase: Mutex::new(Phase::Preparing),
                owner_binding: Mutex::new(None),
                revoked: AtomicBool::new(false),
                revoke_confirmed: AtomicBool::new(false),
            });
            {
                let mut slot = self.session.lock().map_err(|_| Error::Unavailable)?;
                if slot.is_some() {
                    return Err(Error::Invalid);
                }
                *slot = Some(session.clone());
            }
            let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
            match self.send(&session, wire::request::Action::Assess(wire::Empty {})) {
                Ok(wire::response::Result::Accepted(_)) => {
                    *phase = Phase::Prepared;
                    Ok(())
                }
                Ok(_) | Err(_) => {
                    *phase = Phase::Failed;
                    drop(phase);
                    let _ = self.revoke_now(scope);
                    Err(Error::Unavailable)
                }
            }
        })
    }

    fn validate_session<'a>(
        &'a self,
        scope: &'a EnrollmentAuthorizationScope,
        observation: &'a ChallengeObservation,
        grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<ValidatedChallenge, Error>> {
        Box::pin(async move {
            let session = self.session(scope, grant)?;
            let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
            if *phase
                != (Phase::PlanBound {
                    challenge_ref: *observation.challenge_ref(),
                    digest: observation.digest(),
                })
            {
                return Err(Error::Invalid);
            }
            // Once the exchange has started, the lost response does not allow repetition.
            *phase = Phase::Failed;
            let observed_at = self.clock.now();
            let response = self.send(
                &session,
                wire::request::Action::Validate(wire::Validate {
                    challenge: observation.challenge().to_vec(),
                    challenge_ref: observation.challenge_ref().to_vec(),
                    validity_ms: observation.validity_ms(),
                }),
            );
            let Ok(wire::response::Result::Validated(approved)) = response else {
                *phase = Phase::Failed;
                return Err(Error::Unavailable);
            };
            let validated = ValidatedChallenge::issued_for_port(
                *scope,
                observation,
                grant,
                observed_at,
                BootInstant::new(scope.boot, approved.deadline_ns),
            )?;
            validated.check(scope, grant, self.clock.now())?;
            validated.check_observation(observation)?;
            *phase = Phase::Validated {
                challenge_ref: *observation.challenge_ref(),
                digest: observation.digest(),
            };
            Ok(validated)
        })
    }

    fn authorize<'a>(
        &'a self,
        challenge: &'a ValidatedChallenge,
        grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<AuthorizedEnrollment, Error>> {
        Box::pin(async move {
            let scope = challenge.scope();
            let session = self.session(scope, grant)?;
            challenge.check(scope, grant, self.clock.now())?;
            let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
            if *phase
                != (Phase::Validated {
                    challenge_ref: *challenge.challenge_ref(),
                    digest: *challenge.digest(),
                })
            {
                return Err(Error::Invalid);
            }
            *phase = Phase::Failed;
            let response = self.send(
                &session,
                wire::request::Action::Authorize(wire::Authorization {
                    reference: challenge.challenge_ref().to_vec(),
                    challenge_digest: challenge.digest().to_vec(),
                }),
            );
            let Ok(wire::response::Result::Authorized(approved)) = response else {
                *phase = Phase::Failed;
                return Err(Error::Unavailable);
            };
            let reference: [u8; 16] = approved.reference.try_into().map_err(|_| Error::Invalid)?;
            let authorized = AuthorizedEnrollment::issued_for_port_until(
                challenge,
                reference,
                BootInstant::new(scope.boot, approved.deadline_ns),
            )?;
            authorized.check(scope, grant, self.clock.now())?;
            *session
                .owner_binding
                .lock()
                .map_err(|_| Error::Unavailable)? = Some(approved.owner_binding);
            *phase = Phase::Authorized {
                authorization_ref: reference,
                digest: *challenge.digest(),
            };
            Ok(authorized)
        })
    }

    fn revalidate<'a>(
        &'a self,
        authorization: &'a AuthorizedEnrollment,
        scope: &'a EnrollmentAuthorizationScope,
        grant: &'a ValidatedGrant,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            let session = self.session(scope, grant)?;
            authorization.check(scope, grant, self.clock.now())?;
            let mut phase = session.phase.lock().map_err(|_| Error::Unavailable)?;
            if *phase
                != (Phase::Authorized {
                    authorization_ref: *authorization.reference(),
                    digest: *authorization.digest(),
                })
                && *phase
                    != (Phase::Bound {
                        authorization_ref: *authorization.reference(),
                        digest: *authorization.digest(),
                    })
            {
                return Err(Error::Invalid);
            }
            let authorized_phase = *phase;
            *phase = Phase::Failed;
            let response = self.send(
                &session,
                wire::request::Action::Revalidate(wire::Authorization {
                    reference: authorization.reference().to_vec(),
                    challenge_digest: authorization.digest().to_vec(),
                }),
            );
            if !matches!(response, Ok(wire::response::Result::Accepted(_))) {
                *phase = Phase::Failed;
                return Err(Error::Unavailable);
            }
            authorization.check(scope, grant, self.clock.now())?;
            *phase = authorized_phase;
            Ok(())
        })
    }

    fn revoke_now(&self, scope: &EnrollmentAuthorizationScope) -> Result<(), Error> {
        let session = self
            .session
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .ok_or(Error::Unavailable)?;
        if session.scope != *scope {
            return Err(Error::Invalid);
        }
        session.revoked.store(true, Ordering::SeqCst);
        if session.revoke_confirmed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let mut channel = session.control.try_lock().map_err(|_| Error::Unavailable)?;
        if session.revoke_confirmed.load(Ordering::SeqCst) {
            return Ok(());
        }
        match channel.request(
            session.grant.clone(),
            wire::request::Action::Revoke(wire::Empty {}),
            self.control_timeout,
        ) {
            Ok(wire::response::Result::Accepted(_)) => {
                session.revoke_confirmed.store(true, Ordering::SeqCst);
                Ok(())
            }
            _ => Err(Error::Unavailable),
        }
    }

    fn invalidate<'a>(
        &'a self,
        scope: &'a EnrollmentAuthorizationScope,
        _challenge: Option<ValidatedChallenge>,
        _authorization: Option<AuthorizedEnrollment>,
    ) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move { self.revoke_now(scope) })
    }
}

impl Drop for IpcEnrollmentAuthority {
    fn drop(&mut self) {
        let scope = self
            .session
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|session| session.scope));
        if let Some(scope) = scope {
            // An unsuccessful recall does not confirm security; the fence remains the
            // responsibility of the owner of the transaction and live issuance.
            let _ = self.revoke_now(&scope);
        }
    }
}

#[path = "ipc_enrollment_authority.tests.rs"]
#[cfg(test)]
mod tests;
