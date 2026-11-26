use crate::generated::carosign::provider::v1 as provider;
use crate::limits;

use super::super::{WireError, check_version, optional_id, require_id};

/// Checking the provider's envelope.
///
/// Only the first frame, `WorkerHello`, may omit `worker_instance_id`.
/// After `Initialize` It must coincide with the issued - otherwise the command will perform the
/// wrong process.
pub fn check_provider_envelope(frame: &provider::ProviderFrame) -> Result<(), WireError> {
    use provider::provider_frame::Payload as P;

    check_version(frame.major, frame.minor)?;
    if frame.frame_seq == 0 {
        return Err(WireError::BadFrameSequence {
            expected: 1,
            actual: 0,
        });
    }
    if frame.payload.is_none() {
        return Err(WireError::UnexpectedPayload {
            what: "кадр провайдера без сообщения",
        });
    }

    let is_hello = matches!(frame.payload, Some(P::WorkerHello(_)));
    if is_hello {
        if !frame.worker_instance_id.is_empty() {
            // Greeting comes ** before ** issuing an identifier: it cannot be known in
            // advance.
            return Err(WireError::BadId {
                field: "worker_instance_id перед WorkerHello",
            });
        }
    } else {
        require_id(&frame.worker_instance_id, "worker_instance_id")?;
    }
    if matches!(
        frame.payload,
        Some(P::PrebindDescribe(_) | P::PrebindDescription(_))
    ) && !frame.operation_id.is_empty()
    {
        return Err(WireError::UnexpectedPayload {
            what: "декларация не относится к операции",
        });
    }
    if let Some(P::Initialize(initialize)) = frame.payload.as_ref() {
        if initialize.selected_minor != frame.minor {
            return Err(WireError::UnsupportedVersion {
                major: frame.major,
                minor: initialize.selected_minor,
            });
        }
        require_id(
            &initialize.worker_instance_id,
            "initialize.worker_instance_id",
        )?;
        if initialize.worker_instance_id != frame.worker_instance_id {
            return Err(WireError::UnexpectedPayload {
                what: "идентификатор работника не совпадает с конвертом",
            });
        }
        require_id(&initialize.package_id, "initialize.package_id")?;
        require_id(&initialize.profile_id, "initialize.profile_id")?;
        if initialize.enabled_features.len() > limits::FEATURES_MAX {
            return Err(WireError::FieldTooLong {
                field: "initialize.enabled_features",
                actual: initialize.enabled_features.len(),
                allowed: limits::FEATURES_MAX,
            });
        }
        for (index, feature) in initialize.enabled_features.iter().enumerate() {
            if *feature == provider::Feature::Unspecified as i32 {
                return Err(WireError::UnspecifiedEnum {
                    field: "initialize.enabled_features",
                });
            }
            if provider::Feature::from_i32(*feature).is_none() {
                return Err(WireError::UnknownEnum {
                    field: "initialize.enabled_features",
                    value: *feature,
                });
            }
            if initialize.enabled_features[..index].contains(feature) {
                return Err(WireError::UnexpectedPayload {
                    what: "повтор включённой возможности в Initialize",
                });
            }
        }
    }
    match frame.payload.as_ref() {
        Some(
            P::StartVerification(_)
            | P::StartEnrollment(_)
            | P::ContinueEnrollment(_)
            | P::StartRemoval(_)
            | P::ProviderProgress(_)
            | P::VerificationOutcome(_)
            | P::MutationOutcome(_)
            | P::QueryMutation(_)
            | P::QueryMutationResult(_)
            | P::RequestCancel(_)
            | P::CancelReceipt(_)
            | P::EnrollmentTransactionRequest(_)
            | P::EnrollmentTransactionResult(_)
            | P::PrepareEnrollmentChallenge(_)
            | P::EnrollmentChallengeReady(_),
        ) => require_id(&frame.operation_id, "operation_id")?,
        _ => optional_id(&frame.operation_id, "operation_id")?,
    }
    match frame.payload.as_ref() {
        Some(
            P::Describe(_)
            | P::Described(_)
            | P::PrebindDescribe(_)
            | P::PrebindDescription(_)
            | P::Quiesce(_)
            | P::Quiesced(_)
            | P::CloseAuthenticator(_)
            | P::Shutdown(_)
            | P::ProviderProgress(_)
            | P::StartVerification(_)
            | P::StartEnrollment(_)
            | P::ContinueEnrollment(_)
            | P::StartRemoval(_)
            | P::QueryMutation(_)
            | P::RequestCancel(_)
            | P::PrepareEnrollmentChallenge(_)
            | P::CommandAck(_)
            | P::VerificationOutcome(_)
            | P::MutationOutcome(_)
            | P::QueryMutationResult(_)
            | P::CancelReceipt(_)
            | P::EnrollmentTransactionRequest(_)
            | P::EnrollmentTransactionResult(_)
            | P::EnrollmentChallengeReady(_),
        ) => require_id(&frame.command_id, "command_id")?,
        _ => optional_id(&frame.command_id, "command_id")?,
    }
    let subject_command = matches!(
        frame.payload.as_ref(),
        Some(
            P::StartVerification(_)
                | P::StartEnrollment(_)
                | P::ContinueEnrollment(_)
                | P::StartRemoval(_)
                | P::PrepareEnrollmentChallenge(_)
        )
    );
    if subject_command && frame.uid.is_none() {
        return Err(WireError::UnexpectedPayload {
            what: "отсутствует UID субъекта команды",
        });
    }
    if !subject_command && frame.uid.is_some() {
        return Err(WireError::UnexpectedPayload {
            what: "UID не относится к этому сообщению",
        });
    }
    if let Some(P::RequestCancel(cancel)) = frame.payload.as_ref() {
        require_id(&cancel.target_operation_id, "target_operation_id")?;
        if cancel.target_operation_id != frame.operation_id {
            return Err(WireError::UnexpectedPayload {
                what: "идентификатор отменяемой операции не совпадает с конвертом",
            });
        }
    }
    match frame.payload.as_ref() {
        Some(P::EnrollmentTransactionRequest(request)) => {
            check_enrollment_transaction_ids(
                frame,
                &request.mutation_id,
                &request.enrollment_command_id,
            )?;
            if !matches!(
                provider::EnrollmentTransactionAction::from_i32(request.action),
                Some(
                    provider::EnrollmentTransactionAction::EnrollmentTransactionRollback
                        | provider::EnrollmentTransactionAction::EnrollmentTransactionQuery
                )
            ) {
                return Err(WireError::UnexpectedPayload {
                    what: "действие транзакции регистрации",
                });
            }
        }
        Some(P::EnrollmentTransactionResult(result)) => {
            check_enrollment_transaction_ids(
                frame,
                &result.mutation_id,
                &result.enrollment_command_id,
            )?;
            if !matches!(
                provider::EnrollmentTransactionState::from_i32(result.state),
                Some(
                    provider::EnrollmentTransactionState::EnrollmentTransactionRolledBack
                        | provider::EnrollmentTransactionState::EnrollmentTransactionCommitted
                        | provider::EnrollmentTransactionState::EnrollmentTransactionUnknown
                )
            ) {
                return Err(WireError::UnexpectedPayload {
                    what: "состояние транзакции регистрации",
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_enrollment_transaction_ids(
    frame: &provider::ProviderFrame,
    mutation_id: &[u8],
    enrollment_command_id: &[u8],
) -> Result<(), WireError> {
    require_id(mutation_id, "mutation_id")?;
    require_id(enrollment_command_id, "enrollment_command_id")?;
    if frame.command_id == enrollment_command_id {
        return Err(WireError::UnexpectedPayload {
            what: "команда отката или сверки должна иметь собственный ID",
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "envelope.tests.rs"]
mod tests;
