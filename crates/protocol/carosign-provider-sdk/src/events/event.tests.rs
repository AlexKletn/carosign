use super::*;

#[test]
fn enrollment_challenge_retains_context_and_rejects_invalid_observations() {
    use provider::provider_frame::Payload;
    let mut frame = provider::ProviderFrame {
        worker_instance_id: vec![1; 16],
        device_epoch: 1,
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        payload: Some(Payload::EnrollmentChallengeReady(
            provider::EnrollmentChallengeReady {
                mutation_id: vec![4; 16],
                challenge: vec![5, 6],
                challenge_ref: vec![7; 16],
                validity_ms: 20,
            },
        )),
        ..Default::default()
    };
    assert!(
        matches!(parse(&frame), Ok(Event::EnrollmentChallenge { worker, command, operation, mutation, challenge, challenge_ref, validity_ms: 20, .. }) if worker == vec![1;16] && command == vec![2;16] && operation == vec![3;16] && mutation == vec![4;16] && challenge == vec![5,6] && challenge_ref == vec![7;16])
    );
    if let Some(Payload::EnrollmentChallengeReady(value)) = frame.payload.as_mut() {
        value.challenge = vec![1; 65];
    }
    assert!(parse(&frame).is_err());
    if let Some(Payload::EnrollmentChallengeReady(value)) = frame.payload.as_mut() {
        value.challenge = vec![1];
        value.challenge_ref = vec![0; 16];
    }
    assert!(parse(&frame).is_err());
}

#[test]
fn quiescence_error_cannot_become_a_positive_claim() {
    let mut frame = provider::ProviderFrame {
        command_id: vec![1; 16],
        payload: Some(provider::provider_frame::Payload::Quiesced(
            provider::Quiesced {
                physical_quiesce_confirmed: true,
                error: Some(common::Error {
                    code: common::ErrorCode::ErrorUnavailable as i32,
                    safe_message: String::new(),
                }),
            },
        )),
        ..Default::default()
    };
    assert!(parse(&frame).is_err());
    if let Some(provider::provider_frame::Payload::Quiesced(value)) = &mut frame.payload {
        value.physical_quiesce_confirmed = false;
    }
    assert!(matches!(
        parse(&frame),
        Ok(Event::Quiesced { claimed: false })
    ));
}

#[test]
fn private_provider_token_is_bounded_before_event_creation() {
    let frame = |length| provider::ProviderFrame {
        operation_id: vec![1; 16],
        command_id: vec![2; 16],
        payload: Some(provider::provider_frame::Payload::VerificationOutcome(
            provider::VerificationOutcome {
                status: provider::VerificationStatus::VerifyMatched as i32,
                evidence: Some(provider::verification_outcome::Evidence::OpaqueToken(
                    vec![3; length],
                )),
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    assert!(
        matches!(parse(&frame(limits::OPAQUE_TOKEN_MAX)), Ok(Event::Verification { evidence: Some(ProviderEvidence::OpaqueToken(ref token)), .. }) if token.len() == limits::OPAQUE_TOKEN_MAX)
    );
    assert!(parse(&frame(limits::OPAQUE_TOKEN_MAX + 1)).is_err());
}

#[test]
fn token_value_can_be_cleared_without_treating_a_reference_as_token() {
    use zeroize::Zeroize;
    let mut token = ProviderEvidence::OpaqueToken(vec![0xa5; 32]);
    token.zeroize();
    assert!(matches!(&token, ProviderEvidence::OpaqueToken(bytes) if bytes.is_empty()));
    let mut reference = ProviderEvidence::Reference(vec![0x33; 16]);
    reference.zeroize();
    assert!(matches!(&reference, ProviderEvidence::Reference(bytes) if bytes == &[0x33; 16]));
}

#[test]
fn debug_does_not_disclose_evidence_or_recovery_references() {
    let verification = Event::Verification {
        worker: vec![3; 16],
        device_epoch: 1,
        command: vec![4; 16],
        operation: vec![1; 16],
        status: provider::VerificationStatus::VerifyMatched,
        evidence: Some(ProviderEvidence::Reference(vec![42; 16])),
        format_id: String::new(),
        matched_enrollment: None,
        error: None,
    };
    let mutation = Event::Mutation {
        worker: vec![4; 16],
        device_epoch: 1,
        command: vec![5; 16],
        operation: vec![1; 16],
        mutation: vec![2; 16],
        status: provider::MutationStatus::MutationMayHaveOccurred,
        recovery_ref: vec![42; 16],
        provider_ref: vec![43; 16],
        enrollment: None,
    };
    assert_eq!(format!("{verification:?}"), "Verification(..)");
    assert_eq!(format!("{mutation:?}"), "Mutation(..)");
}

#[test]
fn mutation_reference_is_not_silently_discarded() {
    let mut outcome = provider::MutationOutcome {
        mutation_id: vec![2; 16],
        status: provider::MutationStatus::MutationProviderCompleted as i32,
        provider_opaque_ref: vec![3],
        ..Default::default()
    };
    let frame = |outcome| provider::ProviderFrame {
        operation_id: vec![1; 16],
        command_id: vec![9; 16],
        payload: Some(provider::provider_frame::Payload::MutationOutcome(outcome)),
        ..Default::default()
    };
    assert!(
        matches!(parse(&frame(outcome.clone())), Ok(Event::Mutation { provider_ref, .. }) if provider_ref == vec![3])
    );
    outcome.provider_opaque_ref.clear();
    assert!(parse(&frame(outcome)).is_ok());
}

#[test]
fn operation_event_requires_command_identity() {
    let mut frame = provider::ProviderFrame {
        operation_id: vec![1; 16],
        payload: Some(provider::provider_frame::Payload::VerificationOutcome(
            provider::VerificationOutcome {
                status: provider::VerificationStatus::VerifyNoUsableSample as i32,
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame),
        Err(EventError::Wire(WireError::BadId {
            field: "command_id"
        }))
    ));
    frame.command_id = vec![0; 16];
    assert!(parse(&frame).is_err());
    frame.command_id = vec![2; 16];
    assert!(parse(&frame).is_ok());
}

#[test]
fn control_replies_require_command_identity() {
    use provider::provider_frame::Payload;
    for payload in [
        Payload::CancelReceipt(provider::CancelReceipt {
            status: provider::CancelStatus::CancelRequested as i32,
        }),
        Payload::Quiesced(provider::Quiesced {
            physical_quiesce_confirmed: true,
            error: None,
        }),
        Payload::Described(provider::Described {
            capabilities: Some(common::CapabilityDescriptor::default()),
        }),
    ] {
        let mut frame = provider::ProviderFrame {
            operation_id: vec![1; 16],
            payload: Some(payload),
            ..Default::default()
        };
        assert!(matches!(
            parse(&frame),
            Err(EventError::Wire(WireError::BadId {
                field: "command_id"
            }))
        ));
        frame.command_id = vec![2; 16];
        assert!(parse(&frame).is_ok());
    }
}

#[test]
fn mutation_and_query_reject_oversized_provider_refs() {
    let frame = |payload| provider::ProviderFrame {
        operation_id: vec![1; 16],
        command_id: vec![9; 16],
        payload: Some(payload),
        ..Default::default()
    };
    let mutation = |length| {
        provider::provider_frame::Payload::MutationOutcome(provider::MutationOutcome {
            mutation_id: vec![2; 16],
            status: provider::MutationStatus::MutationMayHaveOccurred as i32,
            provider_opaque_ref: vec![3; length],
            ..Default::default()
        })
    };
    assert!(matches!(
        parse(&frame(mutation(limits::OPAQUE_REF_MAX))),
        Ok(Event::Mutation { provider_ref, .. }) if provider_ref.len() == limits::OPAQUE_REF_MAX
    ));
    assert!(parse(&frame(mutation(limits::OPAQUE_REF_MAX + 1))).is_err());
    let query =
        provider::provider_frame::Payload::QueryMutationResult(provider::QueryMutationResult {
            mutation_id: vec![2; 16],
            state: provider::QueryState::QueryUnknown as i32,
            provider_opaque_ref: vec![3; limits::OPAQUE_REF_MAX + 1],
            ..Default::default()
        });
    assert!(parse(&frame(query)).is_err());
}

#[test]
fn mutation_echo_rejects_zero_generation() {
    let frame = provider::ProviderFrame {
        operation_id: vec![1; 16],
        command_id: vec![9; 16],
        payload: Some(provider::provider_frame::Payload::MutationOutcome(
            provider::MutationOutcome {
                mutation_id: vec![2; 16],
                status: provider::MutationStatus::MutationProviderCompleted as i32,
                enrollment: Some(common::EnrollmentRef {
                    enrollment_id: vec![3; 16],
                    authenticator_id: vec![4; 16],
                    generation: 0,
                }),
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame),
        Err(EventError::Wire(WireError::BadId {
            field: "enrollment.generation"
        }))
    ));
}

#[test]
fn query_enrollment_reference_must_have_valid_identity_and_generation() {
    let frame = |enrollment| provider::ProviderFrame {
        operation_id: vec![1; 16],
        command_id: vec![9; 16],
        payload: Some(provider::provider_frame::Payload::QueryMutationResult(
            provider::QueryMutationResult {
                mutation_id: vec![2; 16],
                state: provider::QueryState::QueryConfirmedPresent as i32,
                enrollment: Some(enrollment),
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    let valid = common::EnrollmentRef {
        enrollment_id: vec![3; 16],
        authenticator_id: vec![4; 16],
        generation: 1,
    };
    assert!(matches!(
        parse(&frame(valid.clone())),
        Err(EventError::Wire(WireError::UnexpectedPayload { .. }))
    ));
    assert!(
        parse(&frame(common::EnrollmentRef {
            generation: 0,
            ..valid.clone()
        }))
        .is_err()
    );
    assert!(
        parse(&frame(common::EnrollmentRef {
            authenticator_id: vec![0; 16],
            ..valid
        }))
        .is_err()
    );
}

#[test]
fn query_provider_reference_is_not_silently_discarded() {
    let mut frame = provider::ProviderFrame {
        operation_id: vec![1; 16],
        command_id: vec![9; 16],
        payload: Some(provider::provider_frame::Payload::QueryMutationResult(
            provider::QueryMutationResult {
                mutation_id: vec![2; 16],
                state: provider::QueryState::QueryUnknown as i32,
                provider_opaque_ref: vec![3],
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame),
        Err(EventError::Wire(WireError::UnexpectedPayload { .. }))
    ));
    if let Some(provider::provider_frame::Payload::QueryMutationResult(result)) = &mut frame.payload
    {
        result.provider_opaque_ref.clear();
    }
    assert!(parse(&frame).is_ok());
}

#[test]
fn cancellation_receipt_rejects_unspecified_and_unknown_status() {
    let frame = |status| provider::ProviderFrame {
        command_id: vec![1; 16],
        operation_id: vec![2; 16],
        payload: Some(provider::provider_frame::Payload::CancelReceipt(
            provider::CancelReceipt { status },
        )),
        ..Default::default()
    };
    for status in [0, -1, i32::MAX] {
        assert!(matches!(
            parse(&frame(status)),
            Err(EventError::Enum {
                field: "cancel.status"
            })
        ));
    }
    for status in [
        provider::CancelStatus::CancelRequested,
        provider::CancelStatus::CancelUnsupported,
        provider::CancelStatus::CancelAlreadyTerminal,
    ] {
        assert!(
            matches!(parse(&frame(status as i32)), Ok(Event::CancelReceipt { status: observed }) if observed == status)
        );
    }
}

#[test]
fn provider_fault_requires_valid_bounded_error() {
    let frame = |error| provider::ProviderFrame {
        payload: Some(provider::provider_frame::Payload::ProviderFault(
            provider::ProviderFault { error },
        )),
        ..Default::default()
    };
    assert!(parse(&frame(None)).is_err());
    assert!(parse(&frame(Some(common::Error::default()))).is_err());
    assert!(
        parse(&frame(Some(common::Error {
            code: i32::MAX,
            safe_message: String::new()
        })))
        .is_err()
    );
    assert!(
        parse(&frame(Some(common::Error {
            code: common::ErrorCode::ErrorInternal as i32,
            safe_message: "а".repeat(129)
        })))
        .is_err()
    );
    assert!(matches!(
        parse(&frame(Some(common::Error {
            code: common::ErrorCode::ErrorInternal as i32,
            safe_message: "сбой".into()
        }))),
        Ok(Event::Fault { .. })
    ));
}

#[test]
fn command_ack_requires_a_valid_reason_when_rejected() {
    let frame = |accepted, rejection| provider::ProviderFrame {
        command_id: vec![1; 16],
        payload: Some(provider::provider_frame::Payload::CommandAck(
            provider::CommandAck {
                accepted,
                rejection,
            },
        )),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame(false, None)),
        Err(EventError::Wire(WireError::UnexpectedPayload { .. }))
    ));
    assert!(parse(&frame(false, Some(common::Error::default()))).is_err());
    assert!(
        parse(&frame(
            false,
            Some(common::Error {
                code: i32::MAX,
                safe_message: String::new()
            })
        ))
        .is_err()
    );
    let reason = common::Error {
        code: common::ErrorCode::ErrorBusy as i32,
        safe_message: String::new(),
    };
    assert!(matches!(
        parse(&frame(true, Some(reason.clone()))),
        Err(EventError::Wire(WireError::UnexpectedPayload { .. }))
    ));
    assert!(matches!(
        parse(&frame(false, Some(reason))),
        Ok(Event::Accepted {
            rejection: Some(_),
            ..
        })
    ));
    assert!(matches!(
        parse(&frame(true, None)),
        Ok(Event::Accepted {
            rejection: None,
            ..
        })
    ));
}

#[test]
fn percentages_are_restricted_to_capture_progress() {
    let frame = |kind, percent| provider::ProviderFrame {
        worker_instance_id: vec![7; 16],
        device_epoch: 1,
        operation_id: vec![3; 16],
        command_id: vec![9; 16],
        payload: Some(provider::provider_frame::Payload::ProviderProgress(
            provider::ProviderProgress {
                capture_state: None,
                feedback: None,
                operation_sequence: 1,
                kind,
                percent,
            },
        )),
        ..Default::default()
    };
    let capturing = provider::ProgressKind::ProgressCapturing as i32;
    assert!(parse(&frame(capturing, 0)).is_ok());
    assert!(parse(&frame(capturing, 100)).is_ok());
    assert!(matches!(
        parse(&frame(capturing, 101)),
        Err(EventError::Percent(101))
    ));
    let working = provider::ProgressKind::ProgressWorking as i32;
    assert!(parse(&frame(working, 0)).is_ok());
    assert!(matches!(
        parse(&frame(working, 1)),
        Err(EventError::Wire(WireError::UnexpectedPayload { .. }))
    ));
}

#[test]
fn matched_enrollment_is_not_silently_discarded() {
    let outcome = provider::VerificationOutcome {
        status: provider::VerificationStatus::VerifyMatched as i32,
        matched_enrollment: Some(common::EnrollmentRef {
            enrollment_id: vec![1; 16],
            authenticator_id: vec![2; 16],
            generation: 1,
        }),
        ..Default::default()
    };
    let frame = provider::ProviderFrame {
        operation_id: vec![3; 16],
        command_id: vec![9; 16],
        payload: Some(provider::provider_frame::Payload::VerificationOutcome(
            outcome,
        )),
        ..Default::default()
    };
    assert!(matches!(
        parse(&frame),
        Ok(Event::Verification { matched_enrollment: Some(reference), .. })
            if reference.enrollment_id == vec![1; 16] && reference.generation == 1
    ));
}
