//! Service: client session, greeting, frame boundaries and descriptors.
//!
//! ⚠️ Checks what breaks quietly: greeting first, repeating the greeting, incompatible
//! version, frame more than 64 KiB, descriptors are not there, the limit of connection
//! operations, `TCP` It's not supported at all.

use carosign_linux_ipc::client::accept::Listener;
use carosign_linux_ipc::client::dispatch;
use carosign_linux_ipc::client::observe::{ObserveStart, start};
use carosign_linux_ipc::client::session::{ClientSession, SessionError, SessionPhase, hello_frame};
use carosign_linux_ipc::codec::{ClientRequest, decode};
use carosign_linux_ipc::credentials::PeerCredentials;
use carosign_linux_ipc::frame::{FrameError, check, check_descriptors};
use carosign_linux_ipc::mapping::client::{self, MappingError};
use carosign_protocol::carosign::client::v1 as client_v1;
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::carosign::common::v1 as common;
use prost::Message;

fn peer() -> PeerCredentials {
    PeerCredentials::from_kernel(1234, 1000, 1000).expect("креды")
}

fn session() -> ClientSession {
    ClientSession::new(peer(), [7u8; 16])
}

fn get_health(request: [u8; 16]) -> Vec<u8> {
    client_v1::ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        request_id: request.to_vec(),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::GetHealth(client_v1::GetHealth {})),
    }
    .encode_to_vec()
}

#[test]
fn duplicate_client_payload_is_rejected_before_protobuf_discards_it() {
    let original = get_health([1; 16]);
    for extra in [vec![0xf2, 0x01, 0x00], vec![0x52, 0x00]] {
        let mut bytes = original.clone();
        bytes.extend(extra);
        assert!(decode(&bytes).is_err());
    }
    assert!(decode(&original).is_ok());
}

#[test]
fn duplicate_subject_selector_is_rejected_before_protobuf_merges_it() {
    let frame = client_v1::ClientFrame::decode(begin_verification([1; 16], 3).as_slice()).unwrap();
    let Payload::BeginVerification(request) = frame.payload.clone().unwrap() else {
        panic!("ожидался запрос проверки");
    };
    let original = request.encode_to_vec();
    let mut repeated_message = original.clone();
    repeated_message.extend([0x12, 0x02, 0x08, 0x01]);
    let mut repeated_choice = original.clone();
    let position = repeated_choice
        .windows(4)
        .position(|part| part == [0x12, 0x02, 0x08, 0x01])
        .unwrap();
    repeated_choice.splice(position..position + 4, [0x12, 0x04, 0x08, 0x01, 0x08, 0x01]);
    let mut envelope = frame;
    envelope.payload = None;
    for payload in [repeated_message, repeated_choice] {
        let mut bytes = envelope.encode_to_vec();
        bytes.extend([0xa2, 0x01]);
        prost::encoding::encode_varint(payload.len() as u64, &mut bytes);
        bytes.extend(payload);
        assert!(decode(&bytes).is_err());
    }
    assert!(decode(&begin_verification([1; 16], 3)).is_ok());
}

fn begin_verification(request: [u8; 16], key: u8) -> Vec<u8> {
    client_v1::ClientFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        request_id: request.to_vec(),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::BeginVerification(client_v1::BeginVerification {
            report_matched_enrollment: false,
            enrollment: None,
            idempotency_key: vec![key; 16],
            subject: Some(carosign_protocol::carosign::common::v1::SubjectSelector {
                selector: Some(
                    carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                        true,
                    ),
                ),
            }),
            authenticator_id: vec![2u8; 16],
            purpose_hint: 1,
            authority_request_ref: Vec::new(),
        })),
    }
    .encode_to_vec()
}

#[test]
fn observe_keeps_the_requested_replay_position() {
    for after_sequence in [0, 42, u64::MAX] {
        let frame = client_v1::ClientFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_BASELINE,
            request_id: vec![1; 16],
            operation_id: Vec::new(),
            event_sequence: 0,
            payload: Some(Payload::ObserveOperation(client_v1::ObserveOperation {
                operation_id: vec![2; 16],
                after_sequence,
            })),
        };
        let (_, request) = decode(&frame.encode_to_vec()).unwrap();
        assert!(
            matches!(request, ClientRequest::ObserveOperation { after_sequence: actual, .. } if actual == after_sequence)
        );
        assert!(
            matches!(client::to_domain(&request).unwrap(), Some(client::DomainRequest::Observe { after_sequence: actual, .. }) if actual == after_sequence)
        );
    }
}

#[test]
fn hello_must_come_first() {
    let mut session = session();
    assert_eq!(session.phase(), SessionPhase::AwaitingHello);
    let result = session.accept(&get_health([1u8; 16]));
    assert!(matches!(result, Err(SessionError::HelloExpected)));
}

#[test]
fn hello_negotiates_and_unlocks_requests() {
    let mut session = session();
    let hello = hello_frame([1u8; 16], 4, 4);
    session.accept(&hello).expect("приветствие");
    assert_eq!(session.phase(), SessionPhase::Ready);
    assert_eq!(session.negotiated().map(|value| value.minor), Some(4));
    let ack = client_v1::ClientFrame::decode(session.hello_ack().unwrap().as_slice()).unwrap();
    assert_eq!(ack.request_id, vec![1; 16]);
    assert!(matches!(ack.payload, Some(Payload::HelloAck(_))));

    let (_, request) = decode(&get_health([2u8; 16])).expect("запрос");
    assert!(dispatch::allowed(false, &request));
    session
        .accept(&get_health([2u8; 16]))
        .expect("после приветствия");
}

#[test]
fn second_hello_is_refused() {
    let mut session = session();
    session
        .accept(&hello_frame([1u8; 16], 4, 4))
        .expect("первое");
    assert_eq!(
        session.accept(&hello_frame([2u8; 16], 4, 4)),
        Err(SessionError::AlreadyHelloed)
    );
}

#[test]
fn incompatible_version_is_refused_before_anything() {
    let mut session = session();
    let mut frame =
        client_v1::ClientFrame::decode(&hello_frame([1u8; 16], 4, 4)[..]).expect("разбор");
    if let Some(Payload::Hello(hello)) = &mut frame.payload {
        hello.major = 2;
    }
    // ⚠️ The rejection comes sooner: the envelope is checked before the meaning of the
    // greeting is analyzed, and it is correct - the version is wrong, the rest does not
    // matter.
    assert!(matches!(
        session.accept(&frame.encode_to_vec()),
        Err(SessionError::Request(_))
    ));
}

#[test]
fn every_request_uses_the_negotiated_minor() {
    let mut session = session();
    session.accept(&hello_frame([1; 16], 4, 4)).unwrap();
    let mut frame = client_v1::ClientFrame::decode(&get_health([2; 16])[..]).unwrap();
    for minor in [3, 5] {
        frame.minor = minor;
        assert_eq!(
            session.accept(&frame.encode_to_vec()),
            Err(SessionError::VersionMismatch)
        );
    }
    frame.minor = 4;
    assert!(session.accept(&frame.encode_to_vec()).is_ok());
}

#[test]
fn oversized_and_truncated_frames_are_refused() {
    let big = vec![0u8; carosign_protocol::limits::FRAME_MAX + 1];
    assert!(matches!(
        decode(&big),
        Err(carosign_linux_ipc::codec::RequestError::Wire(
            carosign_protocol::validation::WireError::FrameTooLarge { .. }
        ))
    ));
    assert!(matches!(
        check(&big, false),
        Err(FrameError::TooLarge { .. })
    ));
    assert!(matches!(check(&big, true), Err(FrameError::Truncated)));
}

#[test]
fn descriptors_only_with_the_right_count() {
    assert!(check_descriptors(2, 2).is_ok());
    assert!(matches!(
        check_descriptors(2, 3),
        Err(FrameError::DescriptorMismatch { .. })
    ));
    assert!(!carosign_linux_ipc::fd_slots::may_carry_descriptors(
        "StartVerification"
    ));
    assert!(carosign_linux_ipc::fd_slots::may_carry_descriptors(
        "BindAuthenticator"
    ));
}

#[test]
fn parsing_requests_does_not_reserve_operation_slots() {
    let mut session = session();
    session
        .accept(&hello_frame([1u8; 16], 4, 4))
        .expect("приветствие");
    for i in 0..8u8 {
        session
            .accept(&begin_verification([10 + i; 16], i + 1))
            .expect("влезает");
    }
}

#[test]
fn remote_listener_is_refused() {
    assert!(Listener::new("tcp://0.0.0.0:5000").is_err());
    assert!(Listener::new("relative.sock").is_err());
    let mut listener = Listener::new("/run/carosignd/control.sock").expect("путь");
    assert!(!listener.is_published());
    listener.publish().expect("публикуем");
    assert!(listener.publish().is_err());
}

#[test]
fn mapping_rejects_bad_ids_and_keeps_requests() {
    let (_, request) = decode(&begin_verification([1u8; 16], 3)).expect("запрос");
    let domain = client::to_domain(&request)
        .expect("перевод")
        .expect("запрос");
    assert!(matches!(
        domain,
        client::DomainRequest::BeginVerification { .. }
    ));

    let bad = ClientRequest::BeginVerification {
        report_matched_enrollment: false,
        enrollment: None,
        idempotency_key: vec![0u8; 16],
        authenticator: vec![2u8; 16],
        subject: carosign_protocol::carosign::common::v1::SubjectSelector {
            selector: Some(
                carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                    true,
                ),
            ),
        },
        purpose_hint:
            carosign_protocol::carosign::common::v1::VerificationPurpose::PurposeSessionLogin,
        authority_request_ref: Vec::new(),
    };
    assert_eq!(client::to_domain(&bad), Err(MappingError::BadId));
}

#[test]
fn verification_context_survives_decode_and_mapping_without_becoming_authority() {
    use carosign_protocol::carosign::common::v1::{
        SubjectSelector, VerificationPurpose, subject_selector::Selector,
    };

    for selector in [
        Selector::CurrentSubject(true),
        Selector::AccountHint("private-account".into()),
        Selector::OpaqueAuthoritySelector(vec![9, 8, 7]),
    ] {
        for purpose in 1..=5 {
            let mut frame =
                client_v1::ClientFrame::decode(begin_verification([1; 16], 3).as_slice()).unwrap();
            let Some(Payload::BeginVerification(begin)) = &mut frame.payload else {
                unreachable!()
            };
            begin.subject = Some(SubjectSelector {
                selector: Some(selector.clone()),
            });
            begin.purpose_hint = purpose;
            begin.authority_request_ref = vec![11, 12, 13];
            let (_, request) = decode(&frame.encode_to_vec()).unwrap();
            let ClientRequest::BeginVerification {
                subject,
                purpose_hint,
                authority_request_ref,
                idempotency_key,
                authenticator,
                ..
            } = &request
            else {
                unreachable!()
            };
            assert_eq!(subject.selector, Some(selector.clone()));
            assert_eq!(
                *purpose_hint,
                VerificationPurpose::from_i32(purpose).unwrap()
            );
            assert_eq!(authority_request_ref, &[11, 12, 13]);
            assert_eq!(idempotency_key, &[3; 16]);
            assert_eq!(authenticator, &[2; 16]);
            assert_eq!(format!("{request:?}"), "BeginVerification(..)");
            let domain = client::to_domain(&request).unwrap().unwrap();
            assert_eq!(format!("{domain:?}"), "BeginVerification(..)");
            let client::DomainRequest::BeginVerification {
                subject: mapped_subject,
                purpose_hint: mapped_purpose,
                authority_request_ref: mapped_ref,
                ..
            } = domain
            else {
                unreachable!()
            };
            assert_eq!(&mapped_subject, subject);
            assert_eq!(&mapped_purpose, purpose_hint);
            assert_eq!(&mapped_ref, authority_request_ref);
        }
    }
}

#[test]
fn verification_context_rejects_invalid_subject_and_purpose() {
    use carosign_protocol::carosign::common::v1::{SubjectSelector, subject_selector::Selector};
    let original =
        client_v1::ClientFrame::decode(begin_verification([1; 16], 3).as_slice()).unwrap();
    for selector in [
        None,
        Some(Selector::CurrentSubject(false)),
        Some(Selector::OpaqueAuthoritySelector(vec![1; 257])),
    ] {
        let mut frame = original.clone();
        let Some(Payload::BeginVerification(begin)) = &mut frame.payload else {
            unreachable!()
        };
        begin.subject = Some(SubjectSelector { selector });
        assert!(decode(&frame.encode_to_vec()).is_err());
    }
    for purpose in [0, 6, -1] {
        let mut frame = original.clone();
        let Some(Payload::BeginVerification(begin)) = &mut frame.payload else {
            unreachable!()
        };
        begin.purpose_hint = purpose;
        assert!(decode(&frame.encode_to_vec()).is_err());
    }
}

#[test]
fn request_debug_does_not_disclose_enrollment_label_or_keys() {
    let request = ClientRequest::BeginEnrollment {
        idempotency_key: vec![1; 16],
        authenticator: vec![2; 16],
        label: "private-label".to_owned(),
        subject: common::SubjectSelector {
            selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
        },
        replace: None,
        authority_request_ref: vec![],
    };
    assert_eq!(format!("{request:?}"), "BeginEnrollment(..)");
    let mapped = client::to_domain(&request).unwrap().unwrap();
    assert_eq!(format!("{mapped:?}"), "BeginEnrollment(..)");
}

#[test]
fn enrollment_requests_preserve_subject_authority_and_replacement() {
    let subject = common::SubjectSelector {
        selector: Some(common::subject_selector::Selector::AccountHint(
            "alice".into(),
        )),
    };
    let replacement = common::EnrollmentRef {
        enrollment_id: vec![3; 16],
        authenticator_id: vec![2; 16],
        generation: 7,
    };
    let payloads = [
        Payload::ListEnrollments(client_v1::ListEnrollments {
            subject: Some(subject.clone()),
            authenticator_id: vec![2; 16],
            authority_request_ref: vec![8],
        }),
        Payload::BeginEnrollment(client_v1::BeginEnrollment {
            idempotency_key: vec![1; 16],
            subject: Some(subject.clone()),
            authenticator_id: vec![2; 16],
            label: "label".into(),
            replace: Some(replacement.clone()),
            authority_request_ref: vec![8],
        }),
        Payload::BeginRemoval(client_v1::BeginRemoval {
            idempotency_key: vec![1; 16],
            subject: Some(subject.clone()),
            enrollment: Some(replacement.clone()),
            authority_request_ref: vec![8],
        }),
    ];
    for payload in payloads {
        let frame = client_v1::ClientFrame {
            major: 1,
            minor: 0,
            request_id: vec![4; 16],
            operation_id: vec![],
            event_sequence: 0,
            payload: Some(payload),
        };
        let (_, request) = decode(&frame.encode_to_vec()).unwrap();
        if let Some(domain) = client::to_domain(&request).unwrap() {
            match domain {
                client::DomainRequest::BeginEnrollment {
                    subject: actual,
                    authority_request_ref,
                    replace,
                    ..
                } => {
                    assert_eq!(actual, subject);
                    assert_eq!(authority_request_ref, vec![8]);
                    assert_eq!(replace, Some(replacement.clone()));
                }
                client::DomainRequest::BeginRemoval {
                    subject: actual,
                    authority_request_ref,
                    ..
                } => {
                    assert_eq!(actual, subject);
                    assert_eq!(authority_request_ref, vec![8]);
                }
                _ => panic!("неверный доменный запрос"),
            }
        }
        match request {
            ClientRequest::ListEnrollments {
                subject: actual,
                authority_request_ref,
                ..
            }
            | ClientRequest::BeginRemoval {
                subject: actual,
                authority_request_ref,
                ..
            } => {
                assert_eq!(actual, subject);
                assert_eq!(authority_request_ref, vec![8]);
            }
            ClientRequest::BeginEnrollment {
                subject: actual,
                authority_request_ref,
                replace,
                ..
            } => {
                assert_eq!(actual, subject);
                assert_eq!(authority_request_ref, vec![8]);
                assert_eq!(replace, Some(replacement.clone()));
            }
            _ => panic!("неверный тип запроса"),
        }
    }
}

#[test]
fn observe_reports_gap_explicitly() {
    assert_eq!(
        start(0, Some(9)),
        ObserveStart::GapThenSnapshot { resume_from: 9 }
    );
    assert_eq!(start(9, Some(9)), ObserveStart::From { sequence: 10 });
}
