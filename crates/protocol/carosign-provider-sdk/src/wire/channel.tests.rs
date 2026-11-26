use carosign_protocol::limits::FRAME_MAX;

use super::*;

#[test]
fn parsed_token_is_cleared_and_reference_is_preserved() {
    use carosign_protocol::carosign::provider::v1::{
        VerificationOutcome, verification_outcome::Evidence,
    };
    let make = |evidence| {
        FrameGuard(ProviderFrame {
            payload: Some(Payload::VerificationOutcome(VerificationOutcome {
                evidence: Some(evidence),
                ..Default::default()
            })),
            ..Default::default()
        })
    };
    let mut token = make(Evidence::OpaqueToken(vec![0xa5; 32]));
    token.clear();
    assert!(
        matches!(&token.0.payload, Some(Payload::VerificationOutcome(outcome)) if matches!(&outcome.evidence, Some(Evidence::OpaqueToken(bytes)) if bytes.is_empty()))
    );
    let mut reference = make(Evidence::EvidenceRef(vec![0x33; 16]));
    reference.clear();
    assert!(
        matches!(&reference.0.payload, Some(Payload::VerificationOutcome(outcome)) if matches!(&outcome.evidence, Some(Evidence::EvidenceRef(bytes)) if bytes == &[0x33; 16]))
    );
}

#[test]
fn malformed_tail_leaves_token_under_frame_guard() {
    use carosign_protocol::carosign::provider::v1::{
        VerificationOutcome, verification_outcome::Evidence,
    };
    let input = ProviderFrame {
        payload: Some(Payload::VerificationOutcome(VerificationOutcome {
            evidence: Some(Evidence::OpaqueToken(vec![0xa5; 32])),
            ..Default::default()
        })),
        ..Default::default()
    };
    let mut bytes = input.encode_to_vec();
    bytes.push(0x80);
    let mut frame = FrameGuard(ProviderFrame::default());
    assert!(frame.0.merge(bytes.as_slice()).is_err());
    assert!(
        matches!(&frame.0.payload, Some(Payload::VerificationOutcome(outcome))
            if matches!(&outcome.evidence, Some(Evidence::OpaqueToken(token)) if token.len() == 32))
    );
    frame.clear();
    assert!(
        matches!(&frame.0.payload, Some(Payload::VerificationOutcome(outcome))
            if matches!(&outcome.evidence, Some(Evidence::OpaqueToken(token)) if token.is_empty()))
    );
}

#[test]
fn outgoing_sequence_exhaustion_closes_both_directions() {
    let mut channel = Channel::new();
    channel.next_out = u64::MAX;
    assert_eq!(channel.prepare(hello()), Err(TransportError::Closed));
    assert_eq!(channel.accept(&[]), Err(TransportError::Closed));
}

#[test]
fn channel_rejects_invalid_cancellation_receipts_in_both_directions() {
    use carosign_protocol::carosign::provider::v1 as wire;
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        frame_seq: 1,
        payload: Some(Payload::CancelReceipt(wire::CancelReceipt { status: 0 })),
        ..Default::default()
    };
    let mut sender = Channel::new();
    assert!(sender.prepare(frame.clone()).is_err());
    assert_eq!(sender.next_out(), 1);
    let mut receiver = Channel::new();
    assert!(receiver.accept(&frame.encode_to_vec()).is_err());
    assert_eq!(
        receiver.accept(&frame.encode_to_vec()),
        Err(TransportError::Closed)
    );
}

#[test]
fn channel_rejects_invalid_ack_progress_and_fault() {
    use carosign_protocol::carosign::provider::v1 as wire;
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        frame_seq: 1,
        ..Default::default()
    };
    for payload in [
        Payload::CommandAck(wire::CommandAck {
            accepted: false,
            rejection: None,
        }),
        Payload::ProviderProgress(wire::ProviderProgress {
            capture_state: None,
            feedback: None,
            operation_sequence: 1,
            kind: 0,
            percent: 0,
        }),
        Payload::ProviderFault(wire::ProviderFault { error: None }),
        Payload::Described(wire::Described { capabilities: None }),
    ] {
        let mut invalid = frame.clone();
        invalid.payload = Some(payload);
        let mut sender = Channel::new();
        assert!(sender.prepare(invalid.clone()).is_err());
        assert_eq!(sender.next_out(), 1);
        let mut receiver = Channel::new();
        assert!(receiver.accept(&invalid.encode_to_vec()).is_err());
        assert_eq!(receiver.next_in(), 1);
        assert_eq!(
            receiver.accept(&invalid.encode_to_vec()),
            Err(TransportError::Closed)
        );
    }
}

#[test]
fn channel_checks_verification_without_losing_matched_enrollment() {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    let reference = common::EnrollmentRef {
        enrollment_id: vec![4; 16],
        authenticator_id: vec![5; 16],
        generation: 1,
    };
    let outcome = wire::VerificationOutcome {
        status: wire::VerificationStatus::VerifyMatched as i32,
        matched_enrollment: Some(reference.clone()),
        ..Default::default()
    };
    let mut frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        frame_seq: 1,
        payload: Some(Payload::VerificationOutcome(outcome)),
        uid: None,
    };
    let bytes = Channel::new().prepare(frame.clone()).unwrap();
    let received = Channel::new().accept(&bytes).unwrap();
    assert!(
        matches!(received.payload, Some(Payload::VerificationOutcome(ref outcome)) if outcome.matched_enrollment == Some(reference))
    );
    if let Some(Payload::VerificationOutcome(outcome)) = &mut frame.payload {
        outcome.status = 0;
    }
    assert!(Channel::new().prepare(frame.clone()).is_err());
    assert!(Channel::new().accept(&frame.encode_to_vec()).is_err());
}

#[test]
fn channel_checks_recovery_results_without_dropping_references() {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    let reference = common::EnrollmentRef {
        enrollment_id: vec![4; 16],
        authenticator_id: vec![5; 16],
        generation: 1,
    };
    let mut frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        frame_seq: 1,
        uid: None,
        payload: Some(Payload::QueryMutationResult(wire::QueryMutationResult {
            mutation_id: vec![6; 16],
            state: wire::QueryState::QueryConfirmedPresent as i32,
            enrollment: Some(reference.clone()),
            provider_opaque_ref: vec![7],
            error: None,
        })),
    };
    let bytes = Channel::new().prepare(frame.clone()).unwrap();
    let received = Channel::new().accept(&bytes).unwrap();
    assert!(
        matches!(received.payload, Some(Payload::QueryMutationResult(ref result)) if result.enrollment == Some(reference) && result.provider_opaque_ref == vec![7])
    );
    if let Some(Payload::QueryMutationResult(result)) = &mut frame.payload {
        result.state = 0;
    }
    assert!(Channel::new().prepare(frame.clone()).is_err());
    assert!(Channel::new().accept(&frame.encode_to_vec()).is_err());
}

#[test]
fn mutation_failure_reason_survives_channel_and_is_not_silently_dropped() {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    let error = common::Error {
        code: common::ErrorCode::ErrorUnavailable as i32,
        safe_message: "недоступно".into(),
    };
    let frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        frame_seq: 1,
        uid: None,
        payload: Some(Payload::MutationOutcome(wire::MutationOutcome {
            mutation_id: vec![4; 16],
            status: wire::MutationStatus::MutationFailed as i32,
            error: Some(error.clone()),
            ..Default::default()
        })),
    };
    let bytes = Channel::new().prepare(frame).unwrap();
    let received = Channel::new().accept(&bytes).unwrap();
    assert!(
        matches!(&received.payload, Some(Payload::MutationOutcome(outcome)) if outcome.error == Some(error))
    );
    assert!(crate::events::parse(&received).is_err());
}

#[test]
fn challenge_response_requires_bounded_content_and_valid_ids() {
    use carosign_protocol::carosign::provider::v1 as wire;
    let ready = wire::EnrollmentChallengeReady {
        mutation_id: vec![4; 16],
        challenge: vec![5; 64],
        challenge_ref: vec![6; 16],
        validity_ms: 1,
    };
    let mut frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        frame_seq: 1,
        uid: None,
        payload: Some(Payload::EnrollmentChallengeReady(ready.clone())),
    };
    let bytes = Channel::new().prepare(frame.clone()).unwrap();
    assert!(Channel::new().accept(&bytes).is_ok());
    for invalid in [
        wire::EnrollmentChallengeReady {
            challenge: vec![],
            ..ready.clone()
        },
        wire::EnrollmentChallengeReady {
            challenge: vec![5; 65],
            ..ready.clone()
        },
        wire::EnrollmentChallengeReady {
            challenge_ref: vec![0; 16],
            ..ready.clone()
        },
        wire::EnrollmentChallengeReady {
            mutation_id: vec![],
            ..ready.clone()
        },
        wire::EnrollmentChallengeReady {
            validity_ms: 0,
            ..ready.clone()
        },
    ] {
        frame.payload = Some(Payload::EnrollmentChallengeReady(invalid));
        assert!(Channel::new().prepare(frame.clone()).is_err());
        assert!(Channel::new().accept(&frame.encode_to_vec()).is_err());
    }
}

#[test]
fn challenge_preparation_requires_exact_context_shape() {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    let command = wire::PrepareEnrollmentChallenge {
        authenticator: Some(common::AuthenticatorRef {
            authenticator_id: vec![4; 16],
            device_epoch: 1,
        }),
        subject: Some(common::SubjectRef {
            authority_id: vec![5; 16],
            subject_id: vec![6; 16],
        }),
        mutation_id: vec![7; 16],
    };
    let mut frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        frame_seq: 1,
        uid: Some(0),
        payload: Some(Payload::PrepareEnrollmentChallenge(command.clone())),
    };
    let bytes = Channel::new().prepare(frame.clone()).unwrap();
    assert!(Channel::new().accept(&bytes).is_ok());
    for invalid in [
        wire::PrepareEnrollmentChallenge {
            authenticator: None,
            ..command.clone()
        },
        wire::PrepareEnrollmentChallenge {
            subject: None,
            ..command.clone()
        },
        wire::PrepareEnrollmentChallenge {
            mutation_id: vec![0; 16],
            ..command.clone()
        },
        wire::PrepareEnrollmentChallenge {
            authenticator: Some(common::AuthenticatorRef {
                authenticator_id: vec![4; 16],
                device_epoch: 2,
            }),
            ..command.clone()
        },
        wire::PrepareEnrollmentChallenge {
            subject: Some(common::SubjectRef {
                authority_id: vec![5; 16],
                subject_id: vec![],
            }),
            ..command.clone()
        },
    ] {
        frame.payload = Some(Payload::PrepareEnrollmentChallenge(invalid));
        assert!(Channel::new().prepare(frame.clone()).is_err());
        assert!(Channel::new().accept(&frame.encode_to_vec()).is_err());
    }
}

#[test]
fn duplicate_uid_is_rejected_before_protobuf_can_discard_the_first() {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    for (first, second) in [(1000u32, 0u32), (0, 1000)] {
        let frame = ProviderFrame {
            major: carosign_protocol::limits::MAJOR,
            minor: carosign_protocol::limits::MINOR_BASELINE,
            worker_instance_id: vec![1; 16],
            command_id: vec![2; 16],
            operation_id: vec![3; 16],
            device_epoch: 1,
            uid: Some(first),
            payload: Some(Payload::PrepareEnrollmentChallenge(
                wire::PrepareEnrollmentChallenge {
                    authenticator: Some(common::AuthenticatorRef {
                        authenticator_id: vec![4; 16],
                        device_epoch: 1,
                    }),
                    subject: Some(common::SubjectRef {
                        authority_id: vec![5; 16],
                        subject_id: vec![6; 16],
                    }),
                    mutation_id: vec![7; 16],
                },
            )),
            ..Default::default()
        };
        let mut bytes = Channel::new().prepare(frame).unwrap();
        bytes.push(0x40); // tag 8, varint
        prost::encoding::encode_varint(u64::from(second), &mut bytes);
        assert_eq!(
            Channel::new().accept(&bytes),
            Err(TransportError::Malformed)
        );
    }
}

#[test]
fn channel_rejects_invalid_hello_and_bind_before_advancing_sequence() {
    let mut invalid_hello = hello();
    if let Some(Payload::WorkerHello(value)) = &mut invalid_hello.payload {
        value.features = vec![1, 1];
    }
    let mut sender = Channel::new();
    assert!(sender.prepare(invalid_hello.clone()).is_err());
    assert_eq!(sender.next_out(), 1);
    invalid_hello.frame_seq = 1;
    let mut receiver = Channel::new();
    assert!(receiver.accept(&invalid_hello.encode_to_vec()).is_err());
    assert_eq!(receiver.next_in(), 1);

    let invalid_bind = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 1,
        payload: Some(Payload::BindAuthenticator(
            carosign_protocol::carosign::provider::v1::BindAuthenticator::default(),
        )),
        ..Default::default()
    };
    let mut receiver = Channel::new();
    assert!(receiver.accept(&invalid_bind.encode_to_vec()).is_err());
    assert_eq!(receiver.next_in(), 1);
}

#[test]
fn bind_epoch_must_match_envelope_on_send_and_receive() {
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    let mut frame = ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 1,
        device_epoch: 2,
        payload: Some(Payload::BindAuthenticator(wire::BindAuthenticator {
            authenticator: Some(common::AuthenticatorRef {
                authenticator_id: vec![2; 16],
                device_epoch: 1,
            }),
            profile_id: vec![3; 16],
            lease_id: vec![4; 16],
            fd_slots: Vec::new(),
        })),
        ..Default::default()
    };
    assert!(Channel::new().prepare(frame.clone()).is_err());
    assert!(Channel::new().accept(&frame.encode_to_vec()).is_err());
    frame.device_epoch = 1;
    let bytes = Channel::new().prepare(frame).unwrap();
    assert!(Channel::new().accept(&bytes).is_ok());
}

#[test]
fn ambiguous_oneof_closes_provider_channel() {
    let mut sender = Channel::new();
    let mut duplicate_payload = sender.prepare(hello()).unwrap();
    duplicate_payload.extend([0x52, 0x00]);
    let mut receiver = Channel::new();
    assert!(matches!(
        receiver.accept(&duplicate_payload),
        Err(TransportError::Malformed)
    ));
    assert!(matches!(
        receiver.accept(&duplicate_payload),
        Err(TransportError::Closed)
    ));

    // VerificationOutcomeBoth alternatives of proof in the same body.
    let duplicate_evidence = [0x82, 0x02, 0x06, 0x12, 0x01, 0x01, 0x1a, 0x01, 0x02];
    assert!(matches!(
        check_oneofs(&duplicate_evidence),
        Err(TransportError::Malformed)
    ));
}

#[test]
fn prebind_payloads_cannot_be_combined_with_any_other_payload() {
    for extra_tag in [26u64, 37, 38] {
        let mut bytes = Channel::new().prepare(hello()).unwrap();
        prost::encoding::encode_varint((extra_tag << 3) | 2, &mut bytes);
        bytes.push(0); // An empty body is enough to test oneof.
        let mut receiver = Channel::new();
        assert_eq!(receiver.accept(&bytes), Err(TransportError::Malformed));
        assert_eq!(receiver.accept(&bytes), Err(TransportError::Closed));
    }
    let mut bytes = Vec::new();
    for tag in [37u64, 38] {
        prost::encoding::encode_varint((tag << 3) | 2, &mut bytes);
        bytes.push(0);
    }
    assert_eq!(check_oneofs(&bytes), Err(TransportError::Malformed));
}

fn hello() -> ProviderFrame {
    ProviderFrame {
        major: 1,
        minor: 0,
        worker_instance_id: Vec::new(),
        frame_seq: 0,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::WorkerHello(
            carosign_protocol::carosign::provider::v1::WorkerHello {
                major: 1,
                min_minor: 0,
                max_minor: 0,
                features: Vec::new(),
                implementation_version: "probe".into(),
            },
        )),
    }
}

#[test]
fn sequence_starts_at_one_and_is_strict() {
    let mut out = Channel::new();
    let bytes = out.prepare(hello()).expect("готовится");
    assert_eq!(out.next_out(), 2);

    let mut incoming = Channel::new();
    let accepted = incoming.accept(&bytes).expect("принимается");
    assert_eq!(accepted.frame_seq, 1);
    // Repeating the same shot is a rejection.
    assert!(matches!(
        incoming.accept(&bytes),
        Err(TransportError::Sequence {
            expected: 2,
            actual: 1
        })
    ));
    let next_bytes = out.prepare(hello()).unwrap();
    assert_eq!(incoming.accept(&next_bytes), Err(TransportError::Closed));
    assert_eq!(incoming.prepare(hello()), Err(TransportError::Closed));
}

#[test]
fn oversized_message_never_reaches_the_wire() {
    let mut out = Channel::new();
    let mut frame = hello();
    if let Some(Payload::WorkerHello(hello)) = &mut frame.payload {
        hello.implementation_version =
            "x".repeat(carosign_protocol::limits::IMPLEMENTATION_VERSION_MAX + 1);
    }
    assert!(out.prepare(frame).is_err());
}

#[test]
fn oversized_received_field_is_rejected_without_advancing_sequence() {
    let mut frame = hello();
    frame.frame_seq = 1;
    if let Some(Payload::WorkerHello(hello)) = &mut frame.payload {
        hello.implementation_version =
            "x".repeat(carosign_protocol::limits::IMPLEMENTATION_VERSION_MAX + 1);
    }
    let bytes = frame.encode_to_vec();
    assert!(bytes.len() < FRAME_MAX);
    let mut incoming = Channel::new();
    assert!(matches!(
        incoming.accept(&bytes),
        Err(TransportError::Invalid(WireError::FieldTooLong {
            field: "implementation_version",
            ..
        }))
    ));
    assert_eq!(incoming.next_in(), 1);
    let mut valid = hello();
    valid.frame_seq = 1;
    assert_eq!(
        incoming.accept(&valid.encode_to_vec()),
        Err(TransportError::Closed)
    );
}

#[test]
fn garbage_is_rejected_not_guessed() {
    let mut incoming = Channel::new();
    assert_eq!(incoming.accept(&[1, 2, 3]), Err(TransportError::Malformed));
}

#[test]
fn exhausted_sequence_never_wraps_in_either_direction() {
    let mut outgoing = Channel {
        next_out: u64::MAX,
        next_in: 1,
        closed: false,
    };
    assert_eq!(outgoing.prepare(hello()), Err(TransportError::Closed));
    assert_eq!(outgoing.next_out(), u64::MAX);

    let mut frame = hello();
    frame.frame_seq = u64::MAX;
    let mut incoming = Channel {
        next_out: 1,
        next_in: u64::MAX,
        closed: false,
    };
    assert_eq!(
        incoming.accept(&frame.encode_to_vec()),
        Err(TransportError::Closed)
    );
    assert_eq!(incoming.next_in(), u64::MAX);
}
