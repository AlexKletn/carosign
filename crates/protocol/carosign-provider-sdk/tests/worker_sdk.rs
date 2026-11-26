//! A front worker and wire fixture (P-01...P-09).
//!
//! ⚠️ The worker here is not pretending to be a hardware guy: he's not proving
//! authentication, he's not declaring warranties, he's checking that the SDK is behaving
//! according to document 06: phases, personnel numbers, exactly one outcome, the failure of
//! an outdated era.

use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_provider_sdk::handshake::{self, Phase};
use carosign_provider_sdk::wire::{Channel, Transport, TransportError};
use prost::Message;

fn id(byte: u8) -> Vec<u8> {
    vec![byte; 16]
}

fn authenticator(epoch: u64) -> common::AuthenticatorRef {
    common::AuthenticatorRef {
        authenticator_id: id(1),
        device_epoch: epoch,
    }
}

#[test]
fn initialize_rejects_zero_worker_instance() {
    let initialize = provider::Initialize {
        worker_instance_id: vec![0; 16],
        selected_minor: carosign_protocol::limits::MINOR_BASELINE,
        package_id: id(2),
        profile_id: id(3),
        enabled_features: Vec::new(),
    };
    assert_eq!(
        carosign_provider_sdk::WorkerContext::from_initialize(&initialize),
        Err(carosign_provider_sdk::context::ContextError::MissingInstance)
    );
}

#[test]
fn initialize_rejects_unvalidated_identity_version_and_features() {
    let valid = provider::Initialize {
        worker_instance_id: id(9),
        selected_minor: carosign_protocol::limits::MINOR_BASELINE,
        package_id: id(2),
        profile_id: id(3),
        enabled_features: vec![provider::Feature::BasicOperations as i32],
    };
    let mut variants = Vec::new();
    let mut bad = valid.clone();
    bad.package_id.clear();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.profile_id = vec![0; 16];
    variants.push(bad);
    let mut bad = valid.clone();
    bad.selected_minor = 2;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.selected_minor = carosign_protocol::limits::MINOR_MAX + 1;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.enabled_features.push(i32::MAX);
    variants.push(bad);
    let mut bad = valid.clone();
    bad.enabled_features
        .push(provider::Feature::BasicOperations as i32);
    variants.push(bad);
    for invalid in variants {
        assert!(carosign_provider_sdk::WorkerContext::from_initialize(&invalid).is_err());
    }
    assert!(carosign_provider_sdk::WorkerContext::from_initialize(&valid).is_ok());
}

#[test]
fn invalid_authenticator_does_not_change_worker_context() {
    let mut context =
        carosign_provider_sdk::WorkerContext::from_initialize(&provider::Initialize {
            worker_instance_id: id(9),
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: id(2),
            profile_id: id(3),
            enabled_features: Vec::new(),
        })
        .unwrap();
    for reference in [
        common::AuthenticatorRef {
            authenticator_id: id(1),
            device_epoch: 0,
        },
        common::AuthenticatorRef {
            authenticator_id: vec![0; 16],
            device_epoch: 1,
        },
        common::AuthenticatorRef {
            authenticator_id: vec![1; 15],
            device_epoch: 1,
        },
    ] {
        let bind = provider::BindAuthenticator {
            authenticator: Some(reference),
            lease_id: id(8),
            profile_id: id(3),
            ..Default::default()
        };
        assert_eq!(
            context.bind(&bind),
            Err(carosign_provider_sdk::context::ContextError::MissingAuthenticator)
        );
        assert!(!context.is_bound());
    }
}

#[test]
fn binding_checks_profile_lease_and_slot_shape_before_changing_context() {
    let mut context =
        carosign_provider_sdk::WorkerContext::from_initialize(&provider::Initialize {
            worker_instance_id: id(9),
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: id(2),
            profile_id: id(3),
            enabled_features: vec![],
        })
        .unwrap();
    let valid = provider::BindAuthenticator {
        authenticator: Some(authenticator(1)),
        lease_id: id(8),
        profile_id: id(3),
        ..Default::default()
    };
    let mut foreign = valid.clone();
    foreign.profile_id = id(4);
    let mut no_lease = valid.clone();
    no_lease.lease_id.clear();
    let mut bad_slot = valid.clone();
    bad_slot.fd_slots.push(provider::FdSlot {
        index: 1,
        role: provider::FdRole::Device as i32,
        resource_id: id(5),
    });
    for invalid in [foreign, no_lease, bad_slot] {
        assert!(context.bind(&invalid).is_err());
        assert!(!context.is_bound());
    }
    context.bind(&valid).unwrap();
    assert_eq!(context.authenticator, valid.authenticator);
    let mut replacement = valid.clone();
    replacement.authenticator = Some(common::AuthenticatorRef {
        authenticator_id: id(7),
        device_epoch: 2,
    });
    assert!(context.bind(&replacement).is_err());
    assert_eq!(context.authenticator, valid.authenticator);
    assert!(context.bind(&valid).is_err());
}

#[test]
fn empty_provider_payload_closes_the_channel() {
    let frame = provider::ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: id(9),
        frame_seq: 1,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: None,
    };
    let mut channel = Channel::new();
    assert!(matches!(
        channel.accept(&frame.encode_to_vec()),
        Err(TransportError::Invalid(_))
    ));
    assert_eq!(
        channel.accept(&frame.encode_to_vec()),
        Err(TransportError::Closed)
    );
}

#[test]
fn full_handshake_and_one_command() {
    // P-01: Welcome → Initialize → Initialized → Bind → Ready → Command
    let mut runtime = Channel::new();
    let mut worker = Channel::new();

    let hello = provider::ProviderFrame {
        major: 1,
        minor: 0,
        worker_instance_id: Vec::new(),
        frame_seq: 0,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::WorkerHello(provider::WorkerHello {
            major: 1,
            min_minor: carosign_protocol::limits::MINOR_BASELINE,
            max_minor: carosign_protocol::limits::MINOR_MAX,
            features: vec![provider::Feature::BasicOperations as i32],
            implementation_version: "probe".into(),
        })),
    };
    let bytes = worker.prepare(hello).expect("готовится");
    let received = runtime.accept(&bytes).expect("принимается");
    assert_eq!(handshake::message_name(&received), "WorkerHello");

    // The platform responds to Initialize. The worker phase is Starting.
    let initialize = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: id(9),
        frame_seq: 0,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::Initialize(provider::Initialize {
            worker_instance_id: id(9),
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: id(2),
            profile_id: id(3),
            enabled_features: vec![provider::Feature::BasicOperations as i32],
        })),
    };
    let bytes = runtime.prepare(initialize).expect("готовится");
    let init = worker.accept(&bytes).expect("принимается");
    let phase = handshake::transition(Phase::Starting, handshake::HandshakeStep::SendHello);
    assert_eq!(phase, Phase::Negotiating);
    assert!(handshake::accepts(phase, &init));

    let context = carosign_provider_sdk::WorkerContext::from_initialize(match &init.payload {
        Some(Payload::Initialize(value)) => value,
        _ => panic!("ожидался Initialize"),
    })
    .expect("контекст собран");
    assert_eq!(context.minor, carosign_protocol::limits::MINOR_BASELINE);
    assert!(!context.is_bound());

    // The binding.
    let bind = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: id(9),
        frame_seq: 0,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 1,
        uid: None,
        payload: Some(Payload::BindAuthenticator(provider::BindAuthenticator {
            authenticator: Some(authenticator(1)),
            profile_id: id(3),
            lease_id: id(4),
            fd_slots: Vec::new(),
        })),
    };
    let bytes = runtime.prepare(bind).expect("готовится");
    let bind_frame = worker.accept(&bytes).expect("принимается");
    assert!(handshake::accepts(Phase::Binding, &bind_frame));
    let phase = handshake::transition(
        Phase::Negotiating,
        handshake::HandshakeStep::SendInitialized,
    );
    let phase = handshake::transition(phase, handshake::HandshakeStep::SendReady);
    assert_eq!(phase, Phase::Ready);
}

#[test]
fn command_before_ready_is_refused() {
    // P-05/document 06: message out of its phase - denial.
    let frame = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_SUBJECT_UID,
        worker_instance_id: id(9),
        frame_seq: 5,
        command_id: id(7),
        operation_id: id(8),
        device_epoch: 1,
        uid: Some(1000),
        payload: Some(Payload::StartVerification(provider::StartVerification {
            authenticator: Some(authenticator(1)),
            subject: Some(common::SubjectRef {
                authority_id: id(2),
                subject_id: id(3),
            }),
            purpose: common::VerificationPurpose::PurposeSessionLogin as i32,
            challenge_ref: id(4),
            recipient_ref: id(5),
            permitted_enrollments: Vec::new(),
        })),
    };
    assert!(!handshake::accepts(Phase::Binding, &frame));
    assert!(handshake::accepts(Phase::Ready, &frame));

    // An empty set does not give the worker the right to check all registrations.
    assert!(matches!(
        carosign_provider_sdk::command::parse(&frame),
        Err(carosign_provider_sdk::command::CommandError::Missing { .. })
    ));
}

#[test]
fn verification_command_preserves_scope_and_rejects_invalid_bindings() {
    use carosign_provider_sdk::command::{Command, parse};
    let binding = provider::EnrollmentBinding {
        enrollment: Some(common::EnrollmentRef {
            enrollment_id: id(6),
            authenticator_id: id(1),
            generation: 2,
        }),
        kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
        provider_opaque_ref: vec![7, 8],
        format_id: "mock".into(),
        format_version: 3,
    };
    let frame = provider::ProviderFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_SUBJECT_UID,
        uid: Some(0),
        operation_id: id(8),
        command_id: id(7),
        device_epoch: 1,
        payload: Some(Payload::StartVerification(provider::StartVerification {
            authenticator: Some(authenticator(1)),
            subject: Some(common::SubjectRef {
                authority_id: id(2),
                subject_id: id(3),
            }),
            purpose: common::VerificationPurpose::PurposeSessionLogin as i32,
            challenge_ref: id(4),
            recipient_ref: id(5),
            permitted_enrollments: vec![binding.clone()],
        })),
        ..Default::default()
    };
    let Command::StartVerification {
        uid,
        permitted_enrollments,
        challenge_ref,
        recipient_ref,
        ..
    } = parse(&frame).unwrap()
    else {
        panic!("ожидалась команда проверки");
    };
    assert_eq!(uid, 0);
    assert_eq!(permitted_enrollments, vec![binding]);
    assert_eq!(challenge_ref, id(4));
    assert_eq!(recipient_ref, id(5));
    for variant in 0..7 {
        let mut invalid = frame.clone();
        let Some(Payload::StartVerification(start)) = &mut invalid.payload else {
            unreachable!()
        };
        match variant {
            0 => start.permitted_enrollments.clear(),
            1 => start
                .permitted_enrollments
                .push(start.permitted_enrollments[0].clone()),
            2 => {
                start.permitted_enrollments[0]
                    .enrollment
                    .as_mut()
                    .unwrap()
                    .authenticator_id = id(9)
            }
            3 => {
                start.permitted_enrollments[0]
                    .enrollment
                    .as_mut()
                    .unwrap()
                    .generation = 0
            }
            4 => start.permitted_enrollments[0].kind = 0,
            5 => start.permitted_enrollments[0].provider_opaque_ref = vec![0; 4097],
            6 => invalid.device_epoch = 2,
            _ => unreachable!(),
        }
        assert!(parse(&invalid).is_err(), "вариант {variant}");
    }
}

#[test]
fn second_terminal_for_one_command_is_refused() {
    // Document 06: Exactly one normal score per command (P-03 in meaning).
    let mut ledger = carosign_provider_sdk::dispatch::TerminalLedger::default();
    ledger.record().expect("первый итог");
    assert!(ledger.record().is_err());
}

#[test]
fn stale_epoch_is_refused_before_any_action() {
    // P-07: The obsolete era of failure to hardware.
    let command = carosign_provider_sdk::command::Command::RequestCancel { target: id(8) };
    assert_eq!(
        carosign_provider_sdk::dispatch::admittance(&command, 1, 2),
        carosign_provider_sdk::dispatch::Action::Refuse
    );
}

#[test]
fn matched_is_not_a_terminal_success() {
    // ⚠️ Workers can only say `Matched` with proof.
    let outcome = provider::VerificationOutcome {
        status: provider::VerificationStatus::VerifyMatched as i32,
        evidence: Some(provider::verification_outcome::Evidence::EvidenceRef(id(6))),
        evidence_format_id: "probe".into(),
        error: None,
        matched_enrollment: None,
    };
    let frame = provider::ProviderFrame {
        major: 1,
        minor: 0,
        worker_instance_id: id(9),
        frame_seq: 3,
        command_id: id(7),
        operation_id: id(8),
        device_epoch: 1,
        uid: None,
        payload: Some(Payload::VerificationOutcome(outcome)),
    };
    let event = carosign_provider_sdk::events::parse(&frame).expect("событие");
    assert!(event.is_terminal());
    match event {
        carosign_provider_sdk::events::Event::Verification { evidence, .. } => {
            // Proof is reference, not success.
            assert!(evidence.is_some());
        }
        other => panic!("ожидался итог проверки, а пришёл {other:?}"),
    }
}

#[test]
fn cancellation_is_acknowledged_not_completed() {
    // P-08: Cancellation receipt is not a stop.
    use carosign_provider_sdk::cancellation::CancelState;
    let state = CancelState::Idle.request();
    assert!(!state.is_quiesced());
    assert_eq!(state.receipt(), provider::CancelStatus::CancelRequested);
}

#[test]
fn transport_reports_closed_channel() {
    struct ClosedSide;
    impl Transport for ClosedSide {
        fn receive(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
            Ok(None)
        }
        fn send(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
            Err(TransportError::Closed)
        }
    }
    let mut side = ClosedSide;
    assert_eq!(side.receive(), Ok(None));
    assert_eq!(side.send(b"x"), Err(TransportError::Closed));
}
