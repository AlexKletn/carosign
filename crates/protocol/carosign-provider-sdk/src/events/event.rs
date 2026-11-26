use super::{EventError, ProviderEvidence};

use core::fmt;

use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;
pub use carosign_protocol::carosign::provider::v1::{
    MutationStatus, ProgressKind, VerificationStatus,
};
use carosign_protocol::limits;
use carosign_protocol::validation::{WireError, bounded, require_id};

/// What the worker reports.
#[derive(Clone, PartialEq)]
pub enum Event {
    /// Unverified observation; the response itself does not confirm a trusted session.
    EnrollmentChallenge {
        worker: Vec<u8>,
        device_epoch: u64,
        command: Vec<u8>,
        operation: Vec<u8>,
        mutation: Vec<u8>,
        challenge: Vec<u8>,
        challenge_ref: Vec<u8>,
        validity_ms: u64,
    },
    /// The command is accepted (does not mean that it is executed).
    Accepted {
        /// Command number.
        command: Vec<u8>,
        /// Refusal if not accepted.
        rejection: Option<common::Error>,
    },
    Progress {
        /// An envelope worker.
        worker: Vec<u8>,
        /// The era of the envelope.
        device_epoch: u64,
        /// The envelope command.
        command: Vec<u8>,
        operation: Vec<u8>,
        /// Turn number in operation.
        sequence: u64,
        kind: provider::ProgressKind,
        /// Percentage (for shooting only).
        percent: u32,
        feedback: Option<common::CaptureFeedback>,
        capture_state: Option<common::CaptureState>,
    },
    /// The result of the check.
    Verification {
        /// The worker and the era out of the envelope.
        worker: Vec<u8>,
        /// The age of the tethered device.
        device_epoch: u64,
        /// The envelope command.
        command: Vec<u8>,
        operation: Vec<u8>,
        /// What happened?
        status: provider::VerificationStatus,
        /// Proof, if it matches.
        evidence: Option<ProviderEvidence>,
        /// Material format identifier.
        format_id: String,
        /// Optional matching registration to check the area.
        matched_enrollment: Option<common::EnrollmentRef>,
        /// Reason for the error if present.
        error: Option<common::Error>,
    },
    /// The result of the change.
    Mutation {
        /// Worker from an envelope; checked by session.
        worker: Vec<u8>,
        /// Epoch of the envelope; checked by session.
        device_epoch: u64,
        /// Command from envelope; checked by session.
        command: Vec<u8>,
        operation: Vec<u8>,
        mutation: Vec<u8>,
        /// What happened?
        status: provider::MutationStatus,
        /// Restore link.
        recovery_ref: Vec<u8>,
        /// An unverified provider link for a future registry record.
        provider_ref: Vec<u8>,
        /// Registration of scheduled registration (document 34).
        ///
        /// ⚠️ Mandatory on "completed" for registration; otherwise it is a protocol
        /// violation, not "without details."
        enrollment: Option<carosign_protocol::carosign::common::v1::EnrollmentRef>,
    },
    /// Respond to a secure status request.
    QueryState {
        mutation: Vec<u8>,
        state: provider::QueryState,
    },
    /// Cancellation receipt.
    CancelReceipt {
        /// What happened?
        status: provider::CancelStatus,
    },
    EnrollmentTransactionResult(provider::EnrollmentTransactionResult),
    /// The device calmed down.
    Quiesced {
        /// This is the worker’s **approval: physical safety is confirmed by the mediator,
        /// not him.
        claimed: bool,
    },
    /// Response to the poll.
    Described {
        /// He can do it.
        capabilities: common::CapabilityDescriptor,
    },
    /// Worker error.
    Fault {
        /// What happened?
        error: common::Error,
    },
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnrollmentChallenge { .. } => f.write_str("EnrollmentChallenge(..)"),
            Self::Accepted { .. } => f.write_str("Accepted(..)"),
            Self::Progress { .. } => f.write_str("Progress(..)"),
            Self::Verification { .. } => f.write_str("Verification(..)"),
            Self::Mutation { .. } => f.write_str("Mutation(..)"),
            Self::QueryState { .. } => f.write_str("QueryState(..)"),
            Self::CancelReceipt { .. } => f.write_str("CancelReceipt(..)"),
            Self::EnrollmentTransactionResult(_) => f.write_str("EnrollmentTransactionResult(..)"),
            Self::Quiesced { .. } => f.write_str("Quiesced(..)"),
            Self::Described { .. } => f.write_str("Described(..)"),
            Self::Fault { .. } => f.write_str("Fault(..)"),
        }
    }
}

pub(crate) fn check_error(error: &common::Error) -> Result<(), EventError> {
    if error.code == common::ErrorCode::ErrorUnspecified as i32 {
        return Err(EventError::Wire(WireError::UnspecifiedEnum {
            field: "error.code",
        }));
    }
    if common::ErrorCode::from_i32(error.code).is_none() {
        return Err(EventError::Wire(WireError::UnknownEnum {
            field: "error.code",
            value: error.code,
        }));
    }
    carosign_protocol::validation::bounded_str(
        &error.safe_message,
        "error.safe_message",
        limits::SAFE_MESSAGE_MAX,
    )?;
    Ok(())
}

fn check_enrollment_reference(reference: &common::EnrollmentRef) -> Result<(), EventError> {
    require_id(&reference.enrollment_id, "enrollment.enrollment_id")?;
    require_id(&reference.authenticator_id, "enrollment.authenticator_id")?;
    if reference.generation == 0 {
        return Err(EventError::Wire(WireError::BadId {
            field: "enrollment.generation",
        }));
    }
    Ok(())
}

pub(crate) fn check_query_result(result: &provider::QueryMutationResult) -> Result<(), EventError> {
    require_id(&result.mutation_id, "mutation_id")?;
    bounded(
        &result.provider_opaque_ref,
        "provider_opaque_ref",
        limits::OPAQUE_REF_MAX,
    )?;
    if let Some(error) = &result.error {
        check_error(error)?;
    }
    if let Some(reference) = &result.enrollment {
        check_enrollment_reference(reference)?;
    }
    provider::QueryState::from_i32(result.state)
        .filter(|state| *state != provider::QueryState::Unspecified)
        .ok_or(EventError::Enum {
            field: "query.state",
        })?;
    Ok(())
}

pub(crate) fn check_mutation_outcome(
    outcome: &provider::MutationOutcome,
) -> Result<(), EventError> {
    require_id(&outcome.mutation_id, "mutation_id")?;
    if let Some(error) = &outcome.error {
        check_error(error)?;
    }
    bounded(
        &outcome.recovery_ref,
        "recovery_ref",
        limits::OPAQUE_REF_MAX,
    )?;
    bounded(
        &outcome.provider_opaque_ref,
        "provider_opaque_ref",
        limits::OPAQUE_REF_MAX,
    )?;
    provider::MutationStatus::from_i32(outcome.status)
        .filter(|status| *status != provider::MutationStatus::Unspecified)
        .ok_or(EventError::Enum {
            field: "mutation.status",
        })?;
    if let Some(reference) = &outcome.enrollment {
        check_enrollment_reference(reference)?;
    }
    Ok(())
}

/// Checks and analyzes the frame as an event.
pub fn parse(frame: &provider::ProviderFrame) -> Result<Event, EventError> {
    use provider::provider_frame::Payload as P;

    let Some(payload) = &frame.payload else {
        return Err(EventError::NotAnEvent);
    };

    match payload {
        P::EnrollmentChallengeReady(ready) => {
            require_id(&frame.worker_instance_id, "worker_instance_id")?;
            require_id(&frame.command_id, "command_id")?;
            require_id(&frame.operation_id, "operation_id")?;
            require_id(&ready.mutation_id, "mutation_id")?;
            require_id(&ready.challenge_ref, "challenge_ref")?;
            if frame.device_epoch == 0
                || ready.challenge.is_empty()
                || ready.challenge.len() > 64
                || ready.validity_ms == 0
            {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "неверная форма регистрационного challenge",
                }));
            }
            Ok(Event::EnrollmentChallenge {
                worker: frame.worker_instance_id.clone(),
                device_epoch: frame.device_epoch,
                command: frame.command_id.clone(),
                operation: frame.operation_id.clone(),
                mutation: ready.mutation_id.clone(),
                challenge: ready.challenge.clone(),
                challenge_ref: ready.challenge_ref.clone(),
                validity_ms: ready.validity_ms,
            })
        }
        P::CommandAck(ack) => {
            require_id(&frame.command_id, "command_id")?;
            if !ack.accepted && ack.rejection.is_none() {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "отказ команды без причины",
                }));
            }
            if ack.accepted && ack.rejection.is_some() {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "принятая команда содержит отказ",
                }));
            }
            if let Some(rejection) = &ack.rejection {
                check_error(rejection)?;
            }
            Ok(Event::Accepted {
                command: frame.command_id.clone(),
                rejection: ack.rejection.clone(),
            })
        }
        P::ProviderProgress(progress) => {
            require_id(&frame.worker_instance_id, "worker_instance_id")?;
            require_id(&frame.command_id, "command_id")?;
            require_id(&frame.operation_id, "operation_id")?;
            if frame.device_epoch == 0 {
                return Err(EventError::Wire(WireError::BadId {
                    field: "device_epoch",
                }));
            }
            if progress.operation_sequence == 0 {
                return Err(EventError::Wire(WireError::BadFrameSequence {
                    expected: 1,
                    actual: 0,
                }));
            }
            if let Some(state) = &progress.capture_state
                && (progress.kind != provider::ProgressKind::ProgressWorking as i32
                    || progress.percent != 0
                    || progress.feedback.is_some()
                    || !carosign_protocol::validation::valid_capture_state(state))
            {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "invalid capture state",
                }));
            }
            if let Some(feedback) = &progress.feedback {
                if progress.kind != provider::ProgressKind::ProgressUnspecified as i32
                    || progress.percent != 0
                    || !carosign_protocol::validation::check_capture_feedback(feedback)
                {
                    return Err(EventError::Wire(WireError::UnexpectedPayload {
                        what: "некорректное типизированное событие захвата",
                    }));
                }
            }
            let kind = provider::ProgressKind::from_i32(progress.kind)
                .filter(|kind| {
                    progress.feedback.is_some()
                        || *kind != provider::ProgressKind::ProgressUnspecified
                })
                .ok_or(EventError::Enum {
                    field: "progress.kind",
                })?;
            if progress.percent > 100 {
                return Err(EventError::Percent(progress.percent));
            }
            if kind != provider::ProgressKind::ProgressCapturing && progress.percent != 0 {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "проценты вне съёмки",
                }));
            }
            Ok(Event::Progress {
                feedback: progress.feedback,
                capture_state: progress.capture_state,
                worker: frame.worker_instance_id.clone(),
                device_epoch: frame.device_epoch,
                command: frame.command_id.clone(),
                operation: frame.operation_id.clone(),
                sequence: progress.operation_sequence,
                kind,
                percent: progress.percent,
            })
        }
        P::VerificationOutcome(outcome) => {
            require_id(&frame.command_id, "command_id")?;
            require_id(&frame.operation_id, "operation_id")?;
            carosign_protocol::validation::check_verification_outcome(outcome)?;
            if let Some(error) = &outcome.error {
                check_error(error)?;
            }
            let status = provider::VerificationStatus::from_i32(outcome.status)
                .filter(|status| *status != provider::VerificationStatus::VerifyStatusUnspecified)
                .ok_or(EventError::Enum {
                    field: "verification.status",
                })?;
            let evidence = match &outcome.evidence {
                Some(provider::verification_outcome::Evidence::EvidenceRef(reference)) => {
                    Some(ProviderEvidence::Reference(reference.clone()))
                }
                Some(provider::verification_outcome::Evidence::OpaqueToken(token)) => {
                    bounded(token, "opaque_token", limits::OPAQUE_TOKEN_MAX)?;
                    Some(ProviderEvidence::OpaqueToken(token.clone()))
                }
                None => None,
            };
            Ok(Event::Verification {
                worker: frame.worker_instance_id.clone(),
                device_epoch: frame.device_epoch,
                command: frame.command_id.clone(),
                operation: frame.operation_id.clone(),
                status,
                evidence,
                format_id: outcome.evidence_format_id.clone(),
                matched_enrollment: outcome.matched_enrollment.clone(),
                error: outcome.error.clone(),
            })
        }
        P::MutationOutcome(outcome) => {
            require_id(&frame.command_id, "command_id")?;
            require_id(&frame.operation_id, "operation_id")?;
            check_mutation_outcome(outcome)?;
            if outcome.error.is_some() {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "исход изменения требует сохранения причины отказа",
                }));
            }
            let status = provider::MutationStatus::from_i32(outcome.status)
                .filter(|status| *status != provider::MutationStatus::Unspecified)
                .ok_or(EventError::Enum {
                    field: "mutation.status",
                })?;
            // ⚠️ The echo checks the platform with the plan, it's just a transfer.
            Ok(Event::Mutation {
                worker: frame.worker_instance_id.clone(),
                device_epoch: frame.device_epoch,
                command: frame.command_id.clone(),
                operation: frame.operation_id.clone(),
                mutation: outcome.mutation_id.clone(),
                status,
                recovery_ref: outcome.recovery_ref.clone(),
                provider_ref: outcome.provider_opaque_ref.clone(),
                enrollment: outcome.enrollment.clone(),
            })
        }
        P::QueryMutationResult(result) => {
            require_id(&frame.command_id, "command_id")?;
            require_id(&frame.operation_id, "operation_id")?;
            check_query_result(result)?;
            if result.enrollment.is_some() || !result.provider_opaque_ref.is_empty() {
                return Err(EventError::Wire(WireError::UnexpectedPayload {
                    what: "ответ сверки требует сохранения ссылки регистрации и провайдера",
                }));
            }
            let state = provider::QueryState::from_i32(result.state)
                .filter(|state| *state != provider::QueryState::Unspecified)
                .ok_or(EventError::Enum {
                    field: "query.state",
                })?;
            Ok(Event::QueryState {
                mutation: result.mutation_id.clone(),
                state,
            })
        }
        P::CancelReceipt(receipt) => {
            require_id(&frame.command_id, "command_id")?;
            require_id(&frame.operation_id, "operation_id")?;
            let status = provider::CancelStatus::from_i32(receipt.status)
                .filter(|status| *status != provider::CancelStatus::Unspecified)
                .ok_or(EventError::Enum {
                    field: "cancel.status",
                })?;
            Ok(Event::CancelReceipt { status })
        }
        P::EnrollmentTransactionResult(result) => {
            carosign_protocol::validation::check_provider_envelope(frame)?;
            Ok(Event::EnrollmentTransactionResult(result.clone()))
        }
        P::Quiesced(quiesced) => {
            require_id(&frame.command_id, "command_id")?;
            if let Some(error) = &quiesced.error {
                check_error(error)?;
                if quiesced.physical_quiesce_confirmed {
                    return Err(EventError::Wire(WireError::UnexpectedPayload {
                        what: "покой и ошибка в одном ответе Quiesced",
                    }));
                }
            }
            Ok(Event::Quiesced {
                claimed: quiesced.physical_quiesce_confirmed,
            })
        }
        P::Described(described) => {
            require_id(&frame.command_id, "command_id")?;
            let capabilities = described.capabilities.clone().ok_or(EventError::Wire(
                WireError::UnexpectedPayload {
                    what: "Described без возможностей",
                },
            ))?;
            carosign_protocol::validation::check_declared_assurance(&capabilities)?;
            Ok(Event::Described { capabilities })
        }
        P::ProviderFault(fault) => {
            let error =
                fault
                    .error
                    .as_ref()
                    .ok_or(EventError::Wire(WireError::UnexpectedPayload {
                        what: "сбой провайдера без причины",
                    }))?;
            check_error(error)?;
            Ok(Event::Fault {
                error: error.clone(),
            })
        }
        _ => Err(EventError::NotAnEvent),
    }
}

impl Event {
    /// Whether the event is final.
    ///
    /// ⚠️ `Matched` The final **not* is: the outcome is decided by the platform.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Verification { .. } | Self::Mutation { .. } | Self::Fault { .. }
        )
    }
}

#[path = "event.tests.rs"]
#[cfg(test)]
mod tests;
