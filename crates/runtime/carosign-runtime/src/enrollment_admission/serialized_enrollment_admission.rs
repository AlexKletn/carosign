use super::AdmissionError;
use super::ConsumptionOutcome;
use super::Stage;
use crate::enrollment_authorization::AuthorizedEnrollment;
use crate::enrollment_authorization::EnrollmentAuthorizationScope;
use crate::ports::CommandPlan;
use crate::ports::EnrollmentAuthorization;
use crate::ports::MutationArmRequest;
use crate::ports::MutationChange;
use crate::ports::TimeSource;
use carosign_core::authorization::GrantScope;
use carosign_core::authorization::ValidatedGrant;
use carosign_persistence::journal::ArmedMutationTicket;
use carosign_persistence::store::MetadataStore;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

fn authorized_enrollment_plan(grant: &ValidatedGrant, plan: &CommandPlan) -> bool {
    match (grant.scope(), plan) {
        (
            GrantScope::Enroll { replace: None, .. },
            CommandPlan::Mutate {
                change: MutationChange::Create { .. },
                ..
            },
        ) => true,
        (
            GrantScope::Enroll {
                replace: Some(expected),
                ..
            },
            CommandPlan::Mutate {
                change: MutationChange::Replace { old, .. },
                ..
            },
        ) => old.enrollment == *expected,
        _ => false,
    }
}

/// One copy corresponds to one session and one recipient.
/// Identifiers do not establish trust in the caller or backend.
pub struct SerializedEnrollmentAdmission {
    scope: EnrollmentAuthorizationScope,
    reference: [u8; 16],
    stage: Mutex<Stage>,
    revoke_requested: AtomicBool,
    revoke_sink: Box<dyn Fn() -> bool + Send + Sync>,
}

impl SerializedEnrollmentAdmission {
    /// Check before continuing an already admitted composite call.
    /// A revocation after this test means an uncertain result.
    pub fn revocation_requested(&self) -> bool {
        self.revoke_requested.load(Ordering::SeqCst)
    }
    /// Called by a trusted lineup for a secure link. The designer himself does not verify
    /// authority, provenance, or binding to the actual sink.
    pub fn new(
        scope: EnrollmentAuthorizationScope,
        authorization: &AuthorizedEnrollment,
        revoke_sink: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Result<Self, AdmissionError> {
        if authorization.challenge().scope() != &scope {
            return Err(AdmissionError::Invalid);
        }
        Ok(Self {
            scope,
            reference: *authorization.reference(),
            stage: Mutex::new(Stage::Ready),
            revoke_requested: AtomicBool::new(false),
            revoke_sink: Box::new(revoke_sink),
        })
    }

    /// Performs a synchronous call at the actual impact boundary.
    /// `check_live` It belongs to a trusted lineup and within the same lock, it is required
    /// to verify the source of permission, the relevance of the lease, the review and the
    /// compliance with a specific protected recipient. All log transitions, cancellations and
    /// recalls must be serialized with the same tolerance; the re-shoot below does not in
    /// itself eliminate the external process race. `consume` Do not postpone the action in
    /// another queue after leaving the lock.
    /// Admission begins on the last reading of pending before the call: the review after it
    /// refers to an already made attempt and requires an uncertain outcome, even if the
    /// caller has not yet had time to start work.
    #[allow(clippy::too_many_arguments)]
    pub fn consume(
        &self,
        scope: &EnrollmentAuthorizationScope,
        authorization: &AuthorizedEnrollment,
        grant: &ValidatedGrant,
        clock: &dyn TimeSource,
        store: &dyn MetadataStore,
        request: &MutationArmRequest,
        ticket: ArmedMutationTicket,
        check_live: impl FnOnce() -> bool,
        consume: impl FnOnce(ArmedMutationTicket) -> ConsumptionOutcome,
    ) -> Result<ConsumptionOutcome, AdmissionError> {
        self.consume_inner(
            ticket,
            |ticket| {
                self.validate_request(
                    scope,
                    authorization,
                    grant,
                    clock,
                    store,
                    request,
                    ticket,
                    check_live,
                )
            },
            consume,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_request(
        &self,
        scope: &EnrollmentAuthorizationScope,
        authorization: &AuthorizedEnrollment,
        grant: &ValidatedGrant,
        clock: &dyn TimeSource,
        store: &dyn MetadataStore,
        request: &MutationArmRequest,
        ticket: &ArmedMutationTicket,
        check_live: impl FnOnce() -> bool,
    ) -> bool {
        if self.scope != *scope
            || self.reference != *authorization.reference()
            || authorization.check(scope, grant, clock.now()).is_err()
            || request.operation != scope.operation
            || request.uid != scope.uid
            || request.lease != scope.lease
            || request.logical_plan.subject() != scope.subject
            || request.logical_plan.authenticator() != scope.authenticator.id
            || request.arm.mutation != scope.mutation
            || request.arm.command != scope.command
            || request.arm.worker != scope.worker
            || request.arm.epoch != scope.authenticator.device_epoch
            || !request.matches_ticket(ticket)
            || !authorized_enrollment_plan(grant, &request.plan)
        {
            return false;
        }
        if carosign_persistence::mutation_store::validate_dispatch_ticket_v4(
            store,
            ticket,
            &request.logical_plan,
        )
        .is_err()
        {
            return false;
        }
        let CommandPlan::Mutate {
            authorization:
                EnrollmentAuthorization::Reference {
                    reference,
                    challenge_digest,
                },
            ..
        } = &request.plan
        else {
            return false;
        };
        reference == authorization.reference()
            && challenge_digest == authorization.digest()
            && check_live()
            && authorization.check(scope, grant, clock.now()).is_ok()
            && carosign_persistence::mutation_store::validate_dispatch_ticket_v4(
                store,
                ticket,
                &request.logical_plan,
            )
            .is_ok()
    }

    fn consume_inner<T>(
        &self,
        ticket: T,
        check: impl FnOnce(&T) -> bool,
        consume: impl FnOnce(T) -> ConsumptionOutcome,
    ) -> Result<ConsumptionOutcome, AdmissionError> {
        self.consume_with_completion_hook(ticket, check, consume, || {})
    }

    fn consume_with_completion_hook<T>(
        &self,
        ticket: T,
        check: impl FnOnce(&T) -> bool,
        consume: impl FnOnce(T) -> ConsumptionOutcome,
        after_flush: impl FnOnce(),
    ) -> Result<ConsumptionOutcome, AdmissionError> {
        let mut stage = self.stage.lock().map_err(|_| AdmissionError::Unavailable)?;
        match *stage {
            Stage::Ready => {}
            Stage::Revoked => return Err(AdmissionError::Revoked),
            Stage::Uncertain | Stage::Finished => return Err(AdmissionError::AlreadyUsed),
        }
        if self.revoke_requested.load(Ordering::SeqCst) {
            drop(stage);
            self.flush_pending()?;
            return Err(AdmissionError::Revoked);
        }
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&ticket)));
        let checked = match checked {
            Ok(checked) => checked,
            Err(payload) => {
                *stage = Stage::Uncertain;
                self.revoke_requested.store(true, Ordering::SeqCst);
                drop(stage);
                let _ = self.flush_pending();
                std::panic::resume_unwind(payload)
            }
        };
        if !checked {
            drop(stage);
            self.flush_pending()?;
            return Err(AdmissionError::Invalid);
        }
        if self.revoke_requested.load(Ordering::SeqCst) {
            drop(stage);
            self.flush_pending()?;
            return Err(AdmissionError::Revoked);
        }
        *stage = Stage::Uncertain;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(ticket)));
        if result.is_err() {
            self.revoke_requested.store(true, Ordering::SeqCst);
        }
        if matches!(result, Ok(outcome) if outcome != ConsumptionOutcome::Unknown) {
            *stage = Stage::Finished;
        }
        drop(stage);
        let revoked = self.flush_pending();
        after_flush();
        match result {
            Err(payload) => {
                // Don’t hide the panic; the recorded review has already been delivered if the
                // recipient could confirm it.
                std::panic::resume_unwind(payload)
            }
            Ok(outcome) => match revoked {
                Ok(false) if !self.revoke_requested.load(Ordering::SeqCst) => Ok(outcome),
                Ok(false) => Err(AdmissionError::Unavailable),
                Ok(true) | Err(_) => Err(AdmissionError::Unavailable),
            },
        }
    }

    /// Serializes a recall at the start of a call. If the call is already underway, it brings
    /// back uncertainty without waiting: cancellations should not hang on the sink.
    /// The revocation processor is fixed at the time of creation and is obliged to revoke a
    /// specific link for a limited time. The error does not confirm the review.
    pub fn revoke(&self) -> Result<(), AdmissionError> {
        self.revoke_requested.store(true, Ordering::SeqCst);
        self.flush_pending().map(|_| ())
    }

    fn flush_pending(&self) -> Result<bool, AdmissionError> {
        if !self.revoke_requested.load(Ordering::SeqCst) {
            return Ok(false);
        }
        let mut stage = match self.stage.try_lock() {
            Ok(stage) => stage,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Err(AdmissionError::Unavailable),
        };
        if *stage == Stage::Revoked {
            return Ok(true);
        }
        *stage = Stage::Uncertain;
        let revoked =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.revoke_sink)()))
                .unwrap_or(false);
        if !revoked {
            return Err(AdmissionError::Unavailable);
        }
        *stage = Stage::Revoked;
        Ok(true)
    }
}

impl Drop for SerializedEnrollmentAdmission {
    fn drop(&mut self) {
        // Do not pass the absence of error as proof of the completion of the operation; the
        // owner of the area is obliged to hold the fence until confirmed cleaning.
        let _ = self.revoke();
    }
}

#[path = "serialized_enrollment_admission.tests.rs"]
#[cfg(test)]
mod tests;
