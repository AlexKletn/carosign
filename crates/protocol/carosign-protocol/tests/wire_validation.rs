//! Check wires on real bytes (P-02...P-06).
//!
//! ⚠️ Bytes here are collected from the same scheme as in combat, and not manually:
//! otherwise, the verification would prove agreement with itself.

use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1::{self as provider, provider_frame::Payload};
use carosign_protocol::validation::{
    WireError, check_bind, check_client_frame, check_frame_sequence, check_frame_size,
    check_provider_envelope, check_worker_hello, require_id,
};
use carosign_protocol::{limits, version};
use prost::Message;

#[test]
fn broker_requires_payload_and_valid_scoped_identifiers() {
    use carosign_protocol::carosign::broker::v1::{self as broker, broker_frame::Payload};
    use carosign_protocol::validation::check_broker_frame;
    let mut frame = broker::BrokerFrame {
        major: 1,
        minor: 1,
        frame_seq: 1,
        ..Default::default()
    };
    assert!(check_broker_frame(&frame).is_err());
    frame.payload = Some(Payload::Ready(broker::ServiceReady {
        resource_id: id(1),
        service_schema: 1,
    }));
    assert!(check_broker_frame(&frame).is_ok());
    frame.request_id = id(2);
    assert!(check_broker_frame(&frame).is_err());
    frame.payload = Some(Payload::Call(broker::Call {
        operation_ref: id(3),
        operation_id: vec![1],
        ..Default::default()
    }));
    assert!(check_broker_frame(&frame).is_err());
    if let Some(Payload::Call(call)) = &mut frame.payload {
        call.operation_id = id(4);
        call.mutation_id = id(5);
        call.command_id = id(6);
    }
    let decoded = broker::BrokerFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
    assert!(check_broker_frame(&decoded).is_ok());
}

#[test]
fn broker_external_body_metadata_is_checked_for_calls_and_results() {
    use carosign_protocol::carosign::broker::v1::{self as broker, broker_frame::Payload};
    use carosign_protocol::validation::check_broker_frame;
    let body = broker::ExternalBody {
        offset: 0,
        length: 1,
        file_size: 1,
        sha256: vec![7; 32],
    };
    for result in [false, true] {
        let payload = |body| {
            if result {
                Payload::CallResult(broker::CallResult {
                    external_body: Some(body),
                    ..Default::default()
                })
            } else {
                Payload::Call(broker::Call {
                    operation_ref: id(3),
                    external_body: Some(body),
                    ..Default::default()
                })
            }
        };
        let mut frame = broker::BrokerFrame {
            major: 1,
            minor: 1,
            frame_seq: 1,
            request_id: id(2),
            payload: Some(payload(body.clone())),
        };
        assert!(check_broker_frame(&frame).is_ok());
        frame.minor = 0;
        assert!(check_broker_frame(&frame).is_err());
        frame.minor = 1;
        for invalid in [
            broker::ExternalBody {
                offset: 1,
                ..body.clone()
            },
            broker::ExternalBody {
                length: 0,
                file_size: 0,
                ..body.clone()
            },
            broker::ExternalBody {
                length: 8 * 1024 * 1024 + 1,
                file_size: 8 * 1024 * 1024 + 1,
                ..body.clone()
            },
            broker::ExternalBody {
                file_size: 2,
                ..body.clone()
            },
            broker::ExternalBody {
                sha256: vec![7; 31],
                ..body.clone()
            },
        ] {
            frame.payload = Some(payload(invalid));
            assert!(check_broker_frame(&frame).is_err());
        }
    }
}

#[test]
fn broker_errors_require_known_code_and_bounded_message() {
    use carosign_protocol::carosign::broker::v1::{self as broker, broker_frame::Payload};
    use carosign_protocol::validation::check_broker_frame;
    let mut frame = broker::BrokerFrame {
        major: 1,
        minor: 1,
        frame_seq: 1,
        request_id: id(1),
        payload: Some(Payload::Fault(broker::ServiceFault { error: None })),
    };
    assert!(check_broker_frame(&frame).is_err());
    let mut error = common::Error {
        code: common::ErrorCode::ErrorInvalidMessage as i32,
        safe_message: "отказ".into(),
    };
    for fault in [false, true] {
        let payload = |error| {
            if fault {
                Payload::Fault(broker::ServiceFault { error: Some(error) })
            } else {
                Payload::CallResult(broker::CallResult {
                    error: Some(error),
                    ..Default::default()
                })
            }
        };
        frame.payload = Some(payload(error.clone()));
        assert!(check_broker_frame(&frame).is_ok());
        for code in [0, i32::MAX] {
            error.code = code;
            frame.payload = Some(payload(error.clone()));
            assert!(check_broker_frame(&frame).is_err());
        }
        error.code = common::ErrorCode::ErrorInvalidMessage as i32;
        error.safe_message = "x".repeat(limits::SAFE_MESSAGE_MAX + 1);
        frame.payload = Some(payload(error.clone()));
        assert!(check_broker_frame(&frame).is_err());
        error.safe_message.clear();
    }
}

fn id(byte: u8) -> Vec<u8> {
    vec![byte; 16]
}

fn hello_frame(seq: u64) -> provider::ProviderFrame {
    provider::ProviderFrame {
        major: 1,
        minor: 0,
        worker_instance_id: Vec::new(),
        frame_seq: seq,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::WorkerHello(provider::WorkerHello {
            major: 1,
            min_minor: 0,
            max_minor: 0,
            features: vec![provider::Feature::BasicOperations as i32],
            implementation_version: "probe-0.1".into(),
        })),
    }
}

#[test]
fn valid_hello_round_trips_through_bytes() {
    // P-01: Greetings are collected, disassembled, and tested.
    let frame = hello_frame(1);
    let bytes = frame.encode_to_vec();
    let back = provider::ProviderFrame::decode(&bytes[..]).expect("разбирается");
    check_provider_envelope(&back).expect("конверт верен");
    if let Some(Payload::WorkerHello(hello)) = &back.payload {
        check_worker_hello(hello).expect("приветствие верно");
    } else {
        panic!("ожидалось приветствие");
    }
}

#[test]
fn hello_must_not_carry_instance_id() {
    // Specification 20 permits an empty worker ID only in `WorkerHello`.
    let mut frame = hello_frame(1);
    frame.worker_instance_id = id(1);
    assert!(matches!(
        check_provider_envelope(&frame),
        Err(WireError::BadId { .. })
    ));
}

#[test]
fn initialize_worker_identity_must_match_its_envelope() {
    let mut frame = hello_frame(1);
    frame.worker_instance_id = id(1);
    for instance in [Vec::new(), vec![0; 16], vec![1; 15], id(2)] {
        frame.payload = Some(Payload::Initialize(provider::Initialize {
            worker_instance_id: instance,
            ..Default::default()
        }));
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert!(check_provider_envelope(&decoded).is_err());
    }
    frame.payload = Some(Payload::Initialize(provider::Initialize {
        worker_instance_id: id(1),
        package_id: id(3),
        profile_id: id(4),
        ..Default::default()
    }));
    assert!(check_provider_envelope(&frame).is_ok());
}

#[test]
fn initialize_selected_minor_must_match_envelope() {
    let mut frame = hello_frame(1);
    frame.minor = 3;
    frame.worker_instance_id = id(1);
    frame.payload = Some(Payload::Initialize(provider::Initialize {
        worker_instance_id: id(1),
        package_id: id(2),
        profile_id: id(3),
        selected_minor: 2,
        ..Default::default()
    }));
    assert!(matches!(
        check_provider_envelope(&frame),
        Err(WireError::UnsupportedVersion { .. })
    ));
    if let Some(Payload::Initialize(initialize)) = &mut frame.payload {
        initialize.selected_minor = 3;
    }
    assert!(check_provider_envelope(&frame).is_ok());
}

#[test]
fn initialize_requires_valid_package_and_profile_ids() {
    let mut frame = hello_frame(1);
    frame.worker_instance_id = id(1);
    for invalid in [Vec::new(), vec![0; 16], vec![1; 15], vec![1; 17]] {
        for package_invalid in [true, false] {
            frame.payload = Some(Payload::Initialize(provider::Initialize {
                worker_instance_id: id(1),
                package_id: if package_invalid {
                    invalid.clone()
                } else {
                    id(3)
                },
                profile_id: if package_invalid {
                    id(4)
                } else {
                    invalid.clone()
                },
                ..Default::default()
            }));
            let decoded =
                provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
            assert!(matches!(
                check_provider_envelope(&decoded),
                Err(WireError::BadId { .. })
            ));
        }
    }
}

#[test]
fn initialize_features_are_bounded_and_unique() {
    let mut frame = hello_frame(1);
    frame.worker_instance_id = id(1);
    let initialize = provider::Initialize {
        worker_instance_id: id(1),
        package_id: id(3),
        profile_id: id(4),
        enabled_features: vec![1, 2],
        ..Default::default()
    };
    frame.payload = Some(Payload::Initialize(initialize.clone()));
    assert!(check_provider_envelope(&frame).is_ok());
    for features in [
        vec![0],
        vec![i32::MAX],
        vec![-1],
        vec![1, 1],
        (1..=(limits::FEATURES_MAX as i32 + 1)).collect(),
    ] {
        frame.payload = Some(Payload::Initialize(provider::Initialize {
            enabled_features: features,
            ..initialize.clone()
        }));
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert!(check_provider_envelope(&decoded).is_err());
    }
}

#[test]
fn hello_rejects_duplicate_features_but_allows_unknown_negotiation_hint() {
    let Some(Payload::WorkerHello(mut hello)) = hello_frame(1).payload else {
        panic!("ожидалось приветствие")
    };
    hello.features.push(0);
    assert!(matches!(
        check_worker_hello(&hello),
        Err(WireError::UnspecifiedEnum { field: "features" })
    ));
    hello.features.clear();
    hello.features.push(i32::MAX);
    assert!(check_worker_hello(&hello).is_ok());
    hello.features.push(i32::MAX);
    let decoded = provider::WorkerHello::decode(hello.encode_to_vec().as_slice()).unwrap();
    assert!(matches!(
        check_worker_hello(&decoded),
        Err(WireError::UnexpectedPayload { .. })
    ));
}

#[test]
fn wrong_major_is_refused_before_anything_else() {
    // P-02: Incompatible version - failure before working with hardware
    let mut frame = hello_frame(1);
    frame.major = 2;
    assert!(matches!(
        check_provider_envelope(&frame),
        Err(WireError::UnsupportedVersion { .. })
    ));
    assert!(version::negotiate(2, 0, 0).is_err());
    // ⚠️ We understand only the current junior: the package has not been published, previous
    // issues are not supported.
    assert!(version::negotiate(1, 0, 0).is_err());
    assert_eq!(
        version::negotiate(1, limits::MINOR_BASELINE, limits::MINOR_MAX).map(|n| n.minor),
        Ok(limits::MINOR_MAX)
    );
}

#[test]
fn frame_sequence_gap_closes_session() {
    // P-04: Missing the number is a breakdown, not a "continuation."
    check_frame_sequence(1, 1).expect("первый кадр");
    assert_eq!(
        check_frame_sequence(0, 0),
        Err(WireError::BadFrameSequence {
            expected: 0,
            actual: 0,
        })
    );
    assert!(matches!(
        check_frame_sequence(2, 4),
        Err(WireError::BadFrameSequence { .. })
    ));
    assert!(matches!(
        check_frame_sequence(2, 1),
        Err(WireError::BadFrameSequence { .. })
    ));
}

#[test]
fn provider_envelope_rejects_zero_frame_sequence_after_decoding() {
    let bytes = hello_frame(0).encode_to_vec();
    let frame = provider::ProviderFrame::decode(bytes.as_slice()).unwrap();
    assert_eq!(
        check_provider_envelope(&frame),
        Err(WireError::BadFrameSequence {
            expected: 1,
            actual: 0
        })
    );
    assert!(check_provider_envelope(&hello_frame(1)).is_ok());
}

#[test]
fn verification_command_requires_nonzero_operation_id() {
    let mut frame = hello_frame(1);
    frame.uid = Some(0);
    frame.worker_instance_id = id(9);
    frame.command_id = id(8);
    frame.payload = Some(Payload::StartVerification(
        provider::StartVerification::default(),
    ));
    for operation_id in [Vec::new(), vec![0; 16], vec![1; 15]] {
        frame.operation_id = operation_id;
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            check_provider_envelope(&decoded),
            Err(WireError::BadId {
                field: "operation_id"
            })
        );
    }
    frame.operation_id = id(1);
    assert!(check_provider_envelope(&frame).is_ok());
}

#[test]
fn verification_command_requires_nonzero_command_id() {
    let mut frame = hello_frame(1);
    frame.uid = Some(1000);
    frame.worker_instance_id = id(9);
    frame.operation_id = id(1);
    frame.payload = Some(Payload::StartVerification(
        provider::StartVerification::default(),
    ));
    for command_id in [Vec::new(), vec![0; 16], vec![1; 15]] {
        frame.command_id = command_id;
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            check_provider_envelope(&decoded),
            Err(WireError::BadId {
                field: "command_id"
            })
        );
    }
    frame.command_id = id(2);
    assert!(check_provider_envelope(&frame).is_ok());
}

#[test]
fn cancellation_target_must_match_envelope_operation() {
    let mut frame = hello_frame(1);
    frame.worker_instance_id = id(9);
    frame.command_id = id(2);
    frame.operation_id = id(1);
    for target in [Vec::new(), vec![0; 16], vec![1; 15], id(3)] {
        frame.payload = Some(Payload::RequestCancel(provider::RequestCancel {
            target_operation_id: target,
        }));
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert!(check_provider_envelope(&decoded).is_err());
    }
    frame.payload = Some(Payload::RequestCancel(provider::RequestCancel {
        target_operation_id: id(1),
    }));
    assert!(check_provider_envelope(&frame).is_ok());
}

#[test]
fn query_and_cancel_responses_require_operation_id() {
    let payloads = [
        Payload::QueryMutation(provider::QueryMutation::default()),
        Payload::QueryMutationResult(provider::QueryMutationResult::default()),
        Payload::CancelReceipt(provider::CancelReceipt::default()),
    ];
    for payload in payloads {
        let mut frame = hello_frame(1);
        frame.worker_instance_id = id(9);
        frame.command_id = id(2);
        frame.payload = Some(payload);
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            check_provider_envelope(&decoded),
            Err(WireError::BadId {
                field: "operation_id"
            })
        );
        frame.operation_id = id(1);
        assert!(check_provider_envelope(&frame).is_ok());
    }
}

#[test]
fn control_commands_and_responses_require_command_id() {
    for payload in [
        Payload::Describe(provider::Describe::default()),
        Payload::Described(provider::Described::default()),
        Payload::Quiesce(provider::Quiesce::default()),
        Payload::Quiesced(provider::Quiesced::default()),
        Payload::CloseAuthenticator(provider::CloseAuthenticator::default()),
        Payload::Shutdown(provider::Shutdown::default()),
    ] {
        let mut frame = hello_frame(1);
        frame.worker_instance_id = id(9);
        frame.payload = Some(payload);
        let decoded = provider::ProviderFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            check_provider_envelope(&decoded),
            Err(WireError::BadId {
                field: "command_id"
            })
        );
        frame.command_id = id(2);
        assert!(check_provider_envelope(&frame).is_ok());
    }
}

#[test]
fn zero_and_wrong_length_ids_are_refused() {
    // P-03: Non-zero 16 bytes required.
    assert!(require_id(&id(1), "leaf").is_ok());
    assert!(require_id(&[0u8; 16], "leaf").is_err());
    assert!(require_id(&[1u8; 15], "leaf").is_err());
    assert!(require_id(&[1u8; 17], "leaf").is_err());
    assert!(require_id(&[], "leaf").is_err());
}

#[test]
fn oversized_frame_is_refused_before_parsing() {
    let big = vec![0u8; carosign_protocol::limits::FRAME_MAX + 1];
    assert!(matches!(
        check_frame_size(&big),
        Err(WireError::FrameTooLarge { .. })
    ));
}

#[test]
fn bind_slots_must_cover_indices_exactly_once() {
    // P-06/P-14: indexes 0..N-1, without repeats, descriptors not more than 8.
    let slot = |index: u32, role: provider::FdRole| provider::FdSlot {
        index,
        role: role as i32,
        resource_id: id(2),
    };
    let base = provider::BindAuthenticator {
        authenticator: Some(common::AuthenticatorRef {
            authenticator_id: id(3),
            device_epoch: 1,
        }),
        profile_id: id(4),
        lease_id: id(5),
        fd_slots: vec![
            slot(0, provider::FdRole::Device),
            slot(1, provider::FdRole::MediatedService),
        ],
    };
    check_bind(&base).expect("согласовано");

    let mut zero_epoch = base.clone();
    zero_epoch.authenticator.as_mut().unwrap().device_epoch = 0;
    let decoded =
        provider::BindAuthenticator::decode(zero_epoch.encode_to_vec().as_slice()).unwrap();
    assert_eq!(
        check_bind(&decoded),
        Err(WireError::BadId {
            field: "authenticator.device_epoch"
        })
    );

    let mut repeated = base.clone();
    repeated.fd_slots[1].index = 0;
    assert!(check_bind(&repeated).is_err());

    let mut gap = base.clone();
    gap.fd_slots[1].index = 2;
    assert!(check_bind(&gap).is_err());

    let mut too_many = base.clone();
    too_many.fd_slots = (0..9).map(|i| slot(i, provider::FdRole::Device)).collect();
    assert!(check_bind(&too_many).is_err());

    let mut unspecified = base.clone();
    unspecified.fd_slots[0].role = provider::FdRole::Unspecified as i32;
    assert!(check_bind(&unspecified).is_err());

    for role in [-1, 5, i32::MAX] {
        let mut unknown = base.clone();
        unknown.fd_slots[0].role = role;
        let bytes = unknown.encode_to_vec();
        let decoded = provider::BindAuthenticator::decode(bytes.as_slice()).unwrap();
        assert_eq!(
            check_bind(&decoded),
            Err(WireError::UnknownEnum {
                field: "fd_slots.role",
                value: role
            })
        );
    }

    let mut no_lease = base;
    no_lease.lease_id = Vec::new();
    assert!(check_bind(&no_lease).is_err());
}

#[test]
fn client_frame_requires_request_id_and_bounds_label() {
    use carosign_protocol::carosign::client::v1 as client;
    use client::client_frame::Payload as CP;

    let mut frame = client::ClientFrame {
        major: 1,
        minor: 0,
        request_id: id(1),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(CP::BeginEnrollment(client::BeginEnrollment {
            idempotency_key: id(2),
            subject: Some(common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            }),
            authenticator_id: id(3),
            label: "палец".into(),
            replace: None,
            authority_request_ref: Vec::new(),
        })),
    };
    check_client_frame(&frame).expect("верно");

    if let Some(CP::BeginEnrollment(enrollment)) = &mut frame.payload {
        enrollment.subject = None;
    }
    assert!(check_client_frame(&frame).is_err());
    if let Some(CP::BeginEnrollment(enrollment)) = &mut frame.payload {
        enrollment.subject = Some(common::SubjectSelector {
            selector: Some(common::subject_selector::Selector::CurrentSubject(false)),
        });
    }
    assert!(check_client_frame(&frame).is_err());
    if let Some(CP::BeginEnrollment(enrollment)) = &mut frame.payload {
        enrollment.subject = Some(common::SubjectSelector {
            selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
        });
        enrollment.replace = Some(common::EnrollmentRef {
            enrollment_id: id(4),
            authenticator_id: id(5),
            generation: 1,
        });
    }
    assert!(check_client_frame(&frame).is_err());
    if let Some(CP::BeginEnrollment(enrollment)) = &mut frame.payload {
        enrollment.replace.as_mut().unwrap().authenticator_id = id(3);
        enrollment.replace.as_mut().unwrap().generation = 0;
    }
    assert!(check_client_frame(&frame).is_err());
    if let Some(CP::BeginEnrollment(enrollment)) = &mut frame.payload {
        enrollment.replace.as_mut().unwrap().generation = 1;
    }
    check_client_frame(&frame).expect("замена валидна");

    frame.request_id = Vec::new();
    assert!(matches!(
        check_client_frame(&frame),
        Err(WireError::BadId { .. })
    ));

    frame.request_id = id(1);
    if let Some(CP::BeginEnrollment(enrollment)) = &mut frame.payload {
        enrollment.label = "x".repeat(65);
    }
    assert!(matches!(
        check_client_frame(&frame),
        Err(WireError::FieldTooLong { .. })
    ));
}

#[test]
fn enum_zero_is_refused_where_required() {
    // P-05: `UNSPECIFIED` It is unacceptable where the meaning is necessary.
    let outcome = provider::VerificationOutcome {
        status: 0,
        evidence: None,
        evidence_format_id: String::new(),
        error: None,
        matched_enrollment: None,
    };
    assert!(matches!(
        carosign_protocol::validation::check_verification_outcome(&outcome),
        Err(WireError::UnspecifiedEnum { .. })
    ));
}

#[test]
fn matched_may_omit_evidence_and_non_matched_forbids_it() {
    let matched = provider::VerificationOutcome {
        status: provider::VerificationStatus::VerifyMatched as i32,
        evidence: Some(provider::verification_outcome::Evidence::EvidenceRef(id(9))),
        evidence_format_id: "gf-v1".into(),
        error: None,
        matched_enrollment: None,
    };
    carosign_protocol::validation::check_verification_outcome(&matched).expect("верно");

    let bare = provider::VerificationOutcome {
        status: provider::VerificationStatus::VerifyMatched as i32,
        evidence: None,
        evidence_format_id: "gf-v1".into(),
        error: None,
        matched_enrollment: None,
    };
    assert!(carosign_protocol::validation::check_verification_outcome(&bare).is_ok());

    let noisy = provider::VerificationOutcome {
        status: provider::VerificationStatus::VerifyNotMatched as i32,
        evidence: Some(provider::verification_outcome::Evidence::EvidenceRef(id(9))),
        evidence_format_id: String::new(),
        error: None,
        matched_enrollment: None,
    };
    assert!(carosign_protocol::validation::check_verification_outcome(&noisy).is_err());
}

#[test]
fn unknown_verification_status_is_rejected_after_protobuf_decode() {
    for value in [-1, 6, i32::MAX] {
        let outcome = provider::VerificationOutcome {
            status: value,
            ..Default::default()
        };
        let decoded =
            provider::VerificationOutcome::decode(outcome.encode_to_vec().as_slice()).unwrap();
        assert_eq!(
            carosign_protocol::validation::check_verification_outcome(&decoded),
            Err(WireError::UnknownEnum {
                field: "verification_outcome.status",
                value
            })
        );
    }
}

#[test]
fn matched_outcome_bounds_format_and_validates_enrollment_reference() {
    let valid = provider::VerificationOutcome {
        status: provider::VerificationStatus::VerifyMatched as i32,
        evidence_format_id: "a".repeat(limits::FORMAT_ID_MAX),
        matched_enrollment: Some(common::EnrollmentRef {
            enrollment_id: id(1),
            authenticator_id: id(2),
            generation: 1,
        }),
        ..Default::default()
    };
    assert!(carosign_protocol::validation::check_verification_outcome(&valid).is_ok());
    let mut oversized = valid.clone();
    oversized.evidence_format_id.push('a');
    assert!(carosign_protocol::validation::check_verification_outcome(&oversized).is_err());
    let mut bad_id = valid.clone();
    bad_id.matched_enrollment.as_mut().unwrap().enrollment_id = vec![0; 16];
    assert!(carosign_protocol::validation::check_verification_outcome(&bad_id).is_err());
    let mut bad_generation = valid.clone();
    bad_generation
        .matched_enrollment
        .as_mut()
        .unwrap()
        .generation = 0;
    assert!(carosign_protocol::validation::check_verification_outcome(&bad_generation).is_err());
    let mut not_matched = valid;
    not_matched.status = provider::VerificationStatus::VerifyNotMatched as i32;
    assert!(carosign_protocol::validation::check_verification_outcome(&not_matched).is_err());
}

#[test]
fn scheme_is_compiled_from_proto_not_written_by_hand() {
    // Schemes are in the general project catalog and are collected `prost` assembly.
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proto = manifest.join("../../../contracts/proto/carosign/provider/v1/provider.proto");
    assert!(
        proto.exists(),
        "схема провайдера на месте: {}",
        proto.display()
    );
}
