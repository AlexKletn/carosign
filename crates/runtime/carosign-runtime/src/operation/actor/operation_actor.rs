use super::{ObservationError, OperationMsg, Step};
use crate::operation::mailbox::{Mailbox, MailboxFull, Priority};
use carosign_engine::{
    DomainInput, EffectSpec, OperationState, PlannedEffect, TransitionError, decide,
};

/// Actor of one operation.
#[derive(Debug)]
pub struct OperationActor {
    state: OperationState,
    mailbox: Mailbox<OperationMsg>,
    control_overflowed: bool,
    enrollment_observation: Option<crate::provider::session::EnrollmentOutcome>,
    verification_observation: Option<crate::provider::session::VerificationObservation>,
    verification_bindings: Option<crate::operation::bindings::PermittedSet>,
    matched_transfer_started: bool,
    matched_transfer_digest: Option<[u8; 32]>,
}

impl OperationActor {
    /// A new actor with an initial state.
    pub fn new(state: OperationState) -> Self {
        Self {
            state,
            mailbox: Mailbox::new(),
            control_overflowed: false,
            enrollment_observation: None,
            verification_observation: None,
            matched_transfer_started: false,
            matched_transfer_digest: None,
            verification_bindings: None,
        }
    }

    /// Current state (only for reading outside)
    pub fn state(&self) -> &OperationState {
        &self.state
    }

    /// A set fixed for verification; is not an authority in itself.
    pub fn verification_scope(&self) -> Option<&crate::operation::bindings::PermittedSet> {
        self.verification_bindings.as_ref()
    }

    /// Once fixes the set until the operation is allowed.
    /// The caller is responsible for the origin of the permit and the re-checking of the
    /// policy; matching the fields is not in itself an authority.
    pub fn bind_verification_scope(
        &mut self,
        grant: &carosign_core::authorization::ValidatedGrant,
        permitted: crate::operation::bindings::PermittedSet,
    ) -> Result<(), ObservationError> {
        if self.verification_bindings.is_some() {
            return Err(ObservationError::AlreadyPending);
        }
        let context = &self.state.common.context;
        if !matches!(
            self.state.body,
            carosign_engine::OperationBody::Verification(_)
        ) || self.state.common.phase != carosign_engine::OperationPhase::New
            || self.has_work()
            || self.control_overflowed
            || grant.scope() != &self.state.required_grant_scope()
            || grant.id() != context.owner.binding
            || grant.issuer() != context.owner.authority
            || grant.boot() != context.deadline.boot()
            || permitted
                .bindings()
                .iter()
                .any(|item| item.enrollment.authenticator != context.authenticator.id)
        {
            return Err(ObservationError::ContextMismatch);
        }
        self.verification_bindings = Some(permitted);
        Ok(())
    }

    /// Holds a response until the creation and verification of the evidence material.
    /// The set is taken from the fixed area, not from the transmitter of the response.
    pub fn hold_verification_observation(
        &mut self,
        observation: crate::provider::session::VerificationObservation,
    ) -> Result<(), ObservationError> {
        if self.verification_observation.is_some() {
            return Err(ObservationError::AlreadyPending);
        }
        let carosign_provider_sdk::events::Event::Verification {
            worker,
            device_epoch,
            command,
            operation,
            ..
        } = observation.event()
        else {
            return Err(ObservationError::ContextMismatch);
        };
        let context = &self.state.common.context;
        let permitted = self
            .verification_bindings
            .as_ref()
            .ok_or(ObservationError::ContextMismatch)?;
        if !matches!(
            self.state.body,
            carosign_engine::OperationBody::Verification { .. }
        ) || self.state.common.phase != carosign_engine::OperationPhase::Running
            || self.control_overflowed
            || self.mailbox.has_control()
            || self.state.common.cancel_requested
            || self
                .state
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendVerify)
            || observation.permitted() != permitted
            || permitted
                .bindings()
                .iter()
                .any(|item| item.enrollment.authenticator != context.authenticator.id)
            || operation.as_slice() != context.operation_id.as_bytes()
            || *device_epoch != context.authenticator.device_epoch.value()
            || self.state.common.worker.is_none_or(|bound| {
                bound.instance.as_bytes().as_slice() != worker.as_slice()
                    || bound.device_epoch.value() != *device_epoch
            })
            || self
                .state
                .common
                .command
                .is_none_or(|bound| bound.id.as_bytes().as_slice() != command.as_slice())
        {
            return Err(ObservationError::ContextMismatch);
        }
        self.verification_observation = Some(observation);
        Ok(())
    }

    /// Approval of the provider pending separate verification.
    pub fn verification_observation(
        &self,
    ) -> Option<&crate::provider::session::VerificationObservation> {
        self.verification_observation.as_ref()
    }

    /// Once transfers a match without a token to an independent evaluation.
    /// The approval does not certify the origin and does not allow entry.
    pub fn take_scoped_provider_assertion(
        &mut self,
    ) -> Result<crate::provider::session::ScopedProviderAssertion, ObservationError> {
        use carosign_provider_sdk::events::{Event, VerificationStatus};

        if self.matched_transfer_started
            || self.state.common.cancel_requested
            || self.state.common.phase != carosign_engine::OperationPhase::Running
            || self.has_work()
            || self.has_control_work()
        {
            return Err(ObservationError::ContextMismatch);
        }
        let observation = self
            .verification_observation
            .as_ref()
            .ok_or(ObservationError::NoObservation)?;
        if Some(observation.permitted()) != self.verification_bindings.as_ref()
            || !matches!(
                observation.event(),
                Event::Verification {
                    status: VerificationStatus::VerifyMatched,
                    evidence: None,
                    error: None,
                    ..
                }
            )
        {
            return Err(ObservationError::ContextMismatch);
        }
        // Once the transfer has begun, the lost statement cannot be restored.
        self.matched_transfer_started = true;
        self.verification_observation
            .take()
            .ok_or(ObservationError::NoObservation)?
            .into_scoped_assertion()
            .map_err(|_| ObservationError::ContextMismatch)
    }

    /// Transmits the engine to a limited entry after independent evaluation.
    /// The transition itself does not confirm the assessment: VerifyEvidence remain waiting
    /// until a separate receipt of the inspector.
    pub fn finish_scoped_assertion_transfer(
        &mut self,
        assessment: &crate::ports::ScopedAssertionAssessment,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<Step, ObservationError> {
        let record = assessment.assertion();
        let context = &self.state.common.context;
        if !self.matched_transfer_started
            || self.verification_observation.is_some()
            || self.state.common.cancel_requested
            || self.state.common.phase != carosign_engine::OperationPhase::Running
            || self.has_work()
            || self.has_control_work()
            || self.control_overflowed
            || !assessment.still_valid(now)
            || context.deadline.is_expired_at(now)
            || record.operation != context.operation_id
            || record.subject != context.subject
            || record.authenticator != context.authenticator
            || record.purpose != context.purpose
            || record.deadline != context.deadline
            || self.state.common.worker.is_none_or(|bound| {
                bound.instance != record.worker
                    || bound.device_epoch != record.authenticator.device_epoch
            })
            || self
                .state
                .common
                .command
                .is_none_or(|bound| bound.id != record.command)
            || self.verification_bindings.as_ref().is_none_or(|permitted| {
                permitted.scoped_digest() != record.permitted_digest
                    || permitted
                        .bindings()
                        .iter()
                        .map(|b| b.enrollment)
                        .collect::<Vec<_>>()
                        != record.permitted
            })
        {
            return Err(ObservationError::ContextMismatch);
        }
        self.post(OperationMsg::Domain(DomainInput::ProviderMatched(
            assessment.evidence(),
        )))
        .map_err(|_| ObservationError::MailboxFull)?;
        Ok(self.run_once())
    }

    /// It moves the retained match token once; the absence of an explicit registration does
    /// not prove the limitation of the match by the set.
    /// Loss of returned material means safe failure, not repetition.
    pub fn take_matched_token(&mut self) -> Result<zeroize::Zeroizing<Vec<u8>>, ObservationError> {
        use carosign_provider_sdk::events::{Event, ProviderEvidence, VerificationStatus};
        if self.matched_transfer_started
            || self.state.common.cancel_requested
            || self.state.common.phase != carosign_engine::OperationPhase::Running
            || self.has_work()
            || self.has_control_work()
        {
            return Err(ObservationError::ContextMismatch);
        }
        let observation = self
            .verification_observation
            .as_mut()
            .ok_or(ObservationError::NoObservation)?;
        let permitted = self
            .verification_bindings
            .as_ref()
            .ok_or(ObservationError::ContextMismatch)?;
        if observation.permitted() != permitted {
            return Err(ObservationError::ContextMismatch);
        }
        let Event::Verification {
            worker,
            device_epoch,
            command,
            operation,
            status,
            evidence,
            matched_enrollment,
            error,
            ..
        } = observation.event_mut()
        else {
            return Err(ObservationError::ContextMismatch);
        };
        let context = &self.state.common.context;
        let Some(enrollment) = matched_enrollment else {
            return Err(ObservationError::ContextMismatch);
        };
        if *status != VerificationStatus::VerifyMatched
            || error.is_some()
            || operation.as_slice() != context.operation_id.as_bytes()
            || *device_epoch != context.authenticator.device_epoch.value()
            || self.state.common.worker.is_none_or(|bound| {
                bound.instance.as_bytes().as_slice() != worker.as_slice()
                    || bound.device_epoch.value() != *device_epoch
            })
            || self
                .state
                .common
                .command
                .is_none_or(|bound| bound.id.as_bytes().as_slice() != command.as_slice())
            || !permitted.bindings().iter().any(|item| {
                item.enrollment.id.as_bytes().as_slice() == enrollment.enrollment_id.as_slice()
                    && item.enrollment.authenticator.as_bytes().as_slice()
                        == enrollment.authenticator_id.as_slice()
                    && item.enrollment.authenticator == context.authenticator.id
                    && item.enrollment.generation.value() == enrollment.generation
            })
        {
            return Err(ObservationError::ContextMismatch);
        }
        let Some(ProviderEvidence::OpaqueToken(token)) = evidence.as_mut() else {
            return Err(ObservationError::ContextMismatch);
        };
        if token.is_empty() {
            return Err(ObservationError::ContextMismatch);
        }
        use sha2::{Digest, Sha256};
        self.matched_transfer_digest = Some(Sha256::digest(&*token).into());
        self.matched_transfer_started = true;
        Ok(zeroize::Zeroizing::new(core::mem::take(token)))
    }

    /// It only matches after the owner has accepted the material.
    /// Authority and independent evaluation are still subject to separate audits.
    pub fn finish_matched_transfer(
        &mut self,
        material: &carosign_evidence::material::EvidenceMaterialRef,
    ) -> Result<Step, ObservationError> {
        if !self.matched_transfer_started
            || self.state.common.cancel_requested
            || self.state.common.phase != carosign_engine::OperationPhase::Running
            || self.has_work()
            || self.has_control_work()
            || self.verification_observation.is_none()
            || material.operation() != *self.state.common.context.operation_id.as_bytes()
            || self
                .matched_transfer_digest
                .as_ref()
                .is_none_or(|digest| !material.matches_digest(digest))
        {
            return Err(ObservationError::ContextMismatch);
        }
        let id = carosign_core::ids::EvidenceId::from_bytes(material.id())
            .map_err(|_| ObservationError::ContextMismatch)?;
        self.post(OperationMsg::Domain(DomainInput::ProviderMatched(id)))
            .map_err(|_| ObservationError::MailboxFull)?;
        self.verification_observation = None;
        self.matched_transfer_digest = None;
        Ok(self.run_once())
    }

    /// It transmits an outcome to the engine without a match, and the match is retained until
    /// the proof is created and independently verified.
    pub fn advance_verification_observation(&mut self) -> Result<(), ObservationError> {
        use carosign_provider_sdk::events::{Event, VerificationStatus};
        if self.state.common.phase != carosign_engine::OperationPhase::Running
            || self.state.common.cancel_requested
            || self.control_overflowed
            || self.mailbox.has_control()
        {
            return Err(ObservationError::ContextMismatch);
        }
        let observation = self
            .verification_observation
            .as_ref()
            .ok_or(ObservationError::NoObservation)?;
        let Event::Verification { status, error, .. } = observation.event() else {
            return Err(ObservationError::ContextMismatch);
        };
        // The engine is still accepting a general failure without a detail, and we're keeping
        // the observation with the cause from the operation until it's destroyed, and we're
        // not giving it away as a mismatch; that shouldn't stop the confirmed cleaning.
        let keep_detail = error.is_some();
        if keep_detail && *status != VerificationStatus::VerifyFailed {
            return Err(ObservationError::ProviderDetailUnavailable);
        }
        let input = match status {
            VerificationStatus::VerifyNotMatched => DomainInput::ProviderNotMatched,
            VerificationStatus::VerifyNoUsableSample => DomainInput::ProviderNoUsableSample,
            VerificationStatus::VerifyCancelled => DomainInput::ProviderCancelled,
            VerificationStatus::VerifyFailed => DomainInput::ProviderFailed,
            VerificationStatus::VerifyMatched => return Err(ObservationError::EvidenceRequired),
            VerificationStatus::VerifyStatusUnspecified => {
                return Err(ObservationError::ContextMismatch);
            }
        };
        self.post(OperationMsg::Domain(input))
            .map_err(|_| ObservationError::MailboxFull)?;
        if !keep_detail {
            self.verification_observation = None;
        }
        Ok(())
    }

    /// Holds a session-verified statement until it is recorded in the journal.
    ///
    /// While there is no stable record, this method does not send input to the engine and
    /// does not allow you to change the registry. Losing the process requires checking the
    /// log.
    pub fn hold_enrollment_observation(
        &mut self,
        observation: crate::provider::session::EnrollmentOutcome,
    ) -> Result<(), ObservationError> {
        if self.enrollment_observation.is_some() {
            return Err(ObservationError::AlreadyPending);
        }
        let carosign_engine::OperationBody::Enrollment { state, .. } = &self.state.body else {
            return Err(ObservationError::ContextMismatch);
        };
        let carosign_provider_sdk::events::Event::Mutation {
            worker,
            device_epoch,
            command,
            ..
        } = observation.event()
        else {
            return Err(ObservationError::ContextMismatch);
        };
        let context = &self.state.common.context;
        if self.state.common.phase != carosign_engine::OperationPhase::InFlight
            || context.operation_id != observation.operation()
            || context.authenticator.id != observation.planned().authenticator
            || context.authenticator.device_epoch.value() != *device_epoch
            || self.state.common.worker.is_none_or(|bound| {
                bound.instance.as_bytes().as_slice() != worker.as_slice()
                    || bound.device_epoch.value() != *device_epoch
            })
            || self
                .state
                .common
                .command
                .is_none_or(|bound| bound.id.as_bytes().as_slice() != command.as_slice())
            || state.mutation != observation.mutation()
            || state.planned_new != Some(observation.planned())
        {
            return Err(ObservationError::ContextMismatch);
        }
        self.enrollment_observation = Some(observation);
        Ok(())
    }

    /// Observation waiting for a steady record.
    pub fn enrollment_observation(&self) -> Option<&crate::provider::session::EnrollmentOutcome> {
        self.enrollment_observation.as_ref()
    }

    /// Putting a message.
    pub fn post(&mut self, message: OperationMsg) -> Result<(), MailboxFull> {
        let priority = message.priority();
        if priority == Priority::Control {
            // After cancellation, withdrawal, deadline or loss of an worker, the withheld
            // approval can no longer be transferred; it is the release of the value, not the
            // proof of physical rubbing of the token.
            self.verification_observation = None;
        }
        let result = match priority {
            Priority::Control => self.mailbox.push_control(message),
            Priority::Normal => self.mailbox.push_normal(message),
        };
        if result.is_err() && priority == Priority::Control {
            self.control_overflowed = true;
        }
        result
    }

    /// Takes one message and takes a step.
    ///
    /// ⚠️ Control overflow is checked **to** parsing: if the control queue is full, it is
    /// time to close the operation, not continue.
    pub fn run_once(&mut self) -> Step {
        if self.control_overflowed {
            return Step::Overflow;
        }
        let taken = self.mailbox.take_control_first();
        let Some(message) = taken else {
            // ⚠️ Empty mail is "nothing to do," not "operation is complete," and it's already
            // cost us time to mix the two words.
            return Step::Idle;
        };
        if self.state.common.phase.is_terminal() {
            return Step::Terminal;
        }
        let input = message.into_domain();
        match decide(&self.state, input) {
            Ok(decision) => {
                self.state = decision.next;
                if self.state.common.phase.is_terminal() {
                    // ⚠️ The terminal doesn't wait; the rest of the messages don't mean
                    // anything.
                    return Step::Advanced {
                        state: Box::new(self.state.clone()),
                        effects: decision.effects,
                    };
                }
                Step::Advanced {
                    state: Box::new(self.state.clone()),
                    effects: decision.effects,
                }
            }
            Err(TransitionError::AlreadyTerminal) => Step::Terminal,
            Err(error) => Step::Rejected(error),
        }
    }

    /// If there are pending reports.
    pub fn has_work(&self) -> bool {
        self.mailbox.control_len() + self.mailbox.normal_len() > 0
    }

    /// Whether the control message is waiting or an overflow is recorded.
    pub fn has_control_work(&self) -> bool {
        self.control_overflowed || self.mailbox.has_control()
    }

    /// A list of effects that the engine has requested, but which have not yet been
    /// confirmed.
    pub fn pending(&self) -> &[PlannedEffect] {
        &self.state.common.pending_effects
    }

    /// Does this effect occur among those who are waiting?
    pub fn expects(&self, spec: EffectSpec) -> bool {
        self.state
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == spec)
    }
}

#[path = "operation_actor.tests.rs"]
#[cfg(test)]
mod tests;
