#[path = "provider_verification_tests_fixtures/confirm_cleanup.rs"]
mod confirm_cleanup;
#[path = "provider_verification_tests_fixtures/evaluation_case.rs"]
mod evaluation_case;
#[path = "provider_verification_tests_fixtures/legacy_authority.rs"]
mod legacy_authority;
#[path = "provider_verification_tests_fixtures/owner_authority.rs"]
mod owner_authority;
#[path = "provider_verification_tests_fixtures/test_authority.rs"]
mod test_authority;
#[path = "provider_verification_tests_fixtures/test_software_verifier.rs"]
mod test_software_verifier;

use super::*;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthenticatorId, EnrollmentId};
use carosign_protocol::carosign::{common::v1 as common, provider::v1 as provider};
use carosign_provider_sdk::wire::Channel;
use carosign_runtime::operation::bindings::EnrollmentBinding;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

fn setup() -> (VerificationExchange, OwnedFd, Channel) {
    let mut pair = [-1; 2];
    // SAFETY: The array accommodates both descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, pair.as_mut_ptr()) },
        0
    );
    // SAFETY: The successful call passed both descriptors to the test.
    let (host, peer) = unsafe { (OwnedFd::from_raw_fd(pair[0]), OwnedFd::from_raw_fd(pair[1])) };
    let permitted = PermittedSet::new(vec![EnrollmentBinding {
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([6; 16]).unwrap(),
            authenticator: AuthenticatorId::from_bytes([7; 16]).unwrap(),
            generation: EnrollmentGeneration::FIRST,
        },
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![8],
        format_id: None,
        format_version: 0,
    }])
    .unwrap();
    let frame = ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        uid: Some(1000),
        payload: Some(Payload::StartVerification(provider::StartVerification {
            authenticator: Some(common::AuthenticatorRef {
                authenticator_id: vec![7; 16],
                device_epoch: 1,
            }),
            subject: Some(common::SubjectRef {
                authority_id: vec![4; 16],
                subject_id: vec![5; 16],
            }),
            purpose: common::VerificationPurpose::PurposeSessionLogin as i32,
            challenge_ref: vec![9; 16],
            recipient_ref: vec![10; 16],
            permitted_enrollments: vec![provider::EnrollmentBinding {
                enrollment: Some(common::EnrollmentRef {
                    enrollment_id: vec![6; 16],
                    authenticator_id: vec![7; 16],
                    generation: 1,
                }),
                kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: vec![8],
                format_id: String::new(),
                format_version: 0,
            }],
        })),
    };
    let transport = ProviderTransport::new(host, Channel::new()).unwrap();
    let exchange =
        VerificationExchange::start(transport, frame, permitted, Duration::from_secs(1)).unwrap();
    let mut bytes = vec![0; 65536];
    // SAFETY: The descriptor and buffer remain valid throughout the read call.
    let n = unsafe { libc::recv(peer.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    assert!(n > 0);
    bytes.truncate(n as usize);
    let mut channel = Channel::new();
    channel.accept(&bytes).unwrap();
    (exchange, peer, channel)
}

#[test]
fn tunnel_exchange_delivers_negative_outcome() {
    tunnel_exchange_delivers_outcome(false);
}

#[test]
fn tunnel_exchange_delivers_matched_evidence() {
    tunnel_exchange_delivers_outcome(true);
}

fn tunnel_exchange_delivers_outcome(matched: bool) {
    use carosign_linux_ipc::provider::tunnel::TunnelClient;
    use std::os::unix::process::CommandExt;

    const PEER_FD: &str = "CAROSIGN_VERIFICATION_TEST_PEER_FD";
    if let Some(fd) = std::env::var_os(PEER_FD) {
        let fd: i32 = fd.into_string().unwrap().parse().unwrap();
        // SAFETY: The test parent gave this particular descriptor.
        let peer = unsafe { OwnedFd::from_raw_fd(fd) };
        tunnel_exchange_peer(peer, matched);
        return;
    }
    let mut fds = [-1; 2];
    // SAFETY: The array contains two locations for return descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) },
        0
    );
    // SAFETY: A successful socketpair transfers ownership of both descriptors.
    let (client, peer) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    let peer_fd = peer.as_raw_fd();
    let name = if matched {
        "provider_verification::tests::tunnel_exchange_delivers_matched_evidence"
    } else {
        "provider_verification::tests::tunnel_exchange_delivers_negative_outcome"
    };
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--nocapture"]);
    command.env(PEER_FD, "198");
    // SAFETY: Only async-signal-safe dup2 is performed after fork and before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(peer_fd, 198) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut worker = command.spawn().unwrap();
    drop(peer);
    let tunnel =
        TunnelClient::connect(client, [1; 16], 1, [2; 16], [3; 16], Duration::from_secs(2))
            .unwrap();
    tunnel_exchange_assert_outcome(tunnel, matched);
    assert!(worker.wait().unwrap().success());
}

fn tunnel_exchange_peer(peer: OwnedFd, matched: bool) {
    use carosign_protocol::carosign::broker_relay::v1::{
        CommandResult, Event, Frame, command_result, frame::Body,
    };
    use prost::Message;

    let read = || {
        let mut buf = vec![0u8; 65536];
        // SAFETY: The descriptor and buffer remain valid throughout the call.
        let n = unsafe { libc::recv(peer.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
        assert!(n > 0);
        buf.truncate(n as usize);
        buf
    };
    let write = |bytes: &[u8]| {
        // SAFETY: The descriptor and buffer remain valid throughout the call.
        assert_eq!(
            unsafe { libc::send(peer.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
            bytes.len() as isize
        );
    };
    write(&read());
    let request = Frame::decode(read().as_slice()).unwrap();
    let Some(Body::Command(command)) = request.body else {
        panic!("ожидалась команда")
    };
    let command_frame = command.frame.unwrap();
    let relay = |seq, body| Frame {
        major: 1,
        minor: 0,
        seq,
        worker_instance: vec![1; 16],
        device_epoch: 1,
        body: Some(body),
    };
    write(
        &relay(
            2,
            Body::CommandResult(CommandResult {
                request_id: command.request_id,
                result: Some(command_result::Result::Sent(true)),
            }),
        )
        .encode_to_vec(),
    );
    for (seq, payload) in [
        (
            1,
            Payload::CommandAck(provider::CommandAck {
                accepted: true,
                rejection: None,
            }),
        ),
        (
            2,
            Payload::VerificationOutcome(provider::VerificationOutcome {
                status: if matched {
                    provider::VerificationStatus::VerifyMatched as i32
                } else {
                    provider::VerificationStatus::VerifyNoUsableSample as i32
                },
                evidence: matched
                    .then(|| provider::verification_outcome::Evidence::OpaqueToken(vec![42; 32])),
                matched_enrollment: matched.then(|| common::EnrollmentRef {
                    enrollment_id: vec![6; 16],
                    authenticator_id: vec![7; 16],
                    generation: 1,
                }),
                ..Default::default()
            }),
        ),
    ] {
        let frame = ProviderFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            worker_instance_id: vec![1; 16],
            frame_seq: seq,
            command_id: command_frame.command_id.clone(),
            operation_id: command_frame.operation_id.clone(),
            device_epoch: 1,
            uid: None,
            payload: Some(payload),
        };
        write(&relay(seq + 2, Body::Event(Event { frame: Some(frame) })).encode_to_vec());
    }
}

fn tunnel_exchange_assert_outcome(
    tunnel: carosign_linux_ipc::provider::tunnel::TunnelClient,
    matched: bool,
) {
    let binding = EnrollmentBinding {
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([6; 16]).unwrap(),
            authenticator: AuthenticatorId::from_bytes([7; 16]).unwrap(),
            generation: EnrollmentGeneration::FIRST,
        },
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![8],
        format_id: None,
        format_version: 0,
    };
    let permitted = PermittedSet::new(vec![binding]).unwrap();
    let frame = ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        uid: Some(1000),
        payload: Some(Payload::StartVerification(provider::StartVerification {
            authenticator: Some(common::AuthenticatorRef {
                authenticator_id: vec![7; 16],
                device_epoch: 1,
            }),
            subject: Some(common::SubjectRef {
                authority_id: vec![4; 16],
                subject_id: vec![5; 16],
            }),
            purpose: common::VerificationPurpose::PurposeSessionLogin as i32,
            challenge_ref: vec![9; 16],
            recipient_ref: vec![10; 16],
            permitted_enrollments: vec![provider::EnrollmentBinding {
                enrollment: Some(common::EnrollmentRef {
                    enrollment_id: vec![6; 16],
                    authenticator_id: vec![7; 16],
                    generation: 1,
                }),
                kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: vec![8],
                format_id: String::new(),
                format_version: 0,
            }],
        })),
    };
    let mut exchange =
        VerificationExchange::start_tunnel(tunnel, frame, permitted, Duration::from_secs(2))
            .unwrap();
    assert!(matches!(
        exchange.next(Duration::from_secs(2)),
        Ok(VerificationMessage::Accepted)
    ));
    let Ok(VerificationMessage::Outcome { observation }) = exchange.next(Duration::from_secs(2))
    else {
        panic!("ожидалось наблюдение провайдера");
    };
    let carosign_provider_sdk::events::Event::Verification {
        status,
        evidence,
        matched_enrollment,
        ..
    } = observation.event()
    else {
        panic!("ожидалось событие проверки");
    };
    if matched {
        assert_eq!(*status, provider::VerificationStatus::VerifyMatched);
        assert!(evidence.is_some());
        assert_eq!(
            matched_enrollment.as_ref().unwrap().enrollment_id,
            vec![6; 16]
        );
    } else {
        assert_eq!(*status, provider::VerificationStatus::VerifyNoUsableSample);
        assert!(evidence.is_none());
    }
    assert!(matches!(
        exchange.into_channel(),
        Ok(VerificationTransport::Tunnel(_))
    ));
}

fn send(peer: &OwnedFd, channel: &mut Channel, operation: u8, payload: Payload) {
    let frame = ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![2; 16],
        operation_id: vec![operation; 16],
        device_epoch: 1,
        uid: None,
        payload: Some(payload),
    };
    let bytes = channel.prepare(frame).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(peer.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
}

#[test]
fn foreign_ack_and_outcome_before_ack_close_session() {
    for early_outcome in [false, true] {
        let (mut exchange, peer, mut channel) = setup();
        let payload = if early_outcome {
            Payload::VerificationOutcome(provider::VerificationOutcome {
                status: provider::VerificationStatus::VerifyNoUsableSample as i32,
                ..Default::default()
            })
        } else {
            Payload::CommandAck(provider::CommandAck {
                accepted: true,
                rejection: None,
            })
        };
        send(
            &peer,
            &mut channel,
            if early_outcome { 3 } else { 11 },
            payload,
        );
        assert!(matches!(
            exchange.next(Duration::from_secs(1)),
            Err(VerificationError::Protocol)
        ));
        assert_eq!(exchange.phase(), VerificationPhase::Failed);
        assert!(exchange.into_transport().is_err());
    }
}

#[test]
fn duplicate_ack_cannot_replace_active_command() {
    let (mut exchange, peer, mut channel) = setup();
    let ack = || {
        Payload::CommandAck(provider::CommandAck {
            accepted: true,
            rejection: None,
        })
    };
    send(&peer, &mut channel, 3, ack());
    assert!(matches!(
        exchange.next(Duration::from_secs(1)),
        Ok(VerificationMessage::Accepted)
    ));
    send(&peer, &mut channel, 3, ack());
    assert!(matches!(
        exchange.next(Duration::from_secs(1)),
        Err(VerificationError::Protocol)
    ));
    assert_eq!(exchange.phase(), VerificationPhase::Failed);
}

#[test]
fn exchange_delivers_ack_and_negative_outcome_to_matching_actor() {
    exercise_actor_delivery(false, false, false, false, false, EvaluationCase::Normal);
}

#[test]
fn rejected_command_is_retained_until_actor_and_effect_owner_accept_it() {
    exercise_actor_delivery(true, false, false, false, false, EvaluationCase::Normal);
}

#[test]
fn matched_evidence_reaches_client_only_after_verification_and_cleanup() {
    exercise_actor_delivery(false, true, false, false, false, EvaluationCase::Normal);
}

#[test]
fn matched_without_enrollment_cannot_become_evidence() {
    exercise_actor_delivery(false, true, true, false, false, EvaluationCase::Normal);
}

#[test]
fn scoped_matched_without_token_is_taken_once_without_authentication() {
    exercise_actor_delivery(
        false,
        true,
        true,
        false,
        false,
        EvaluationCase::ScopedAssertion,
    );
}

#[test]
fn matched_material_cannot_be_replaced_by_another_token() {
    exercise_actor_delivery(false, true, false, true, false, EvaluationCase::Normal);
}

#[test]
fn matched_revoked_before_handoff_cannot_reach_verifier() {
    exercise_actor_delivery(false, true, false, false, true, EvaluationCase::Normal);
}

#[test]
fn independent_assessment_cannot_substitute_enrollment() {
    exercise_actor_delivery(
        false,
        true,
        false,
        false,
        false,
        EvaluationCase::WrongAssessment,
    );
}

#[test]
fn revoked_after_evaluation_cannot_deliver_positive_receipt() {
    exercise_actor_delivery(false, true, false, false, false, EvaluationCase::Revoked);
}

#[test]
fn interrupted_verifier_is_not_automatically_retried() {
    exercise_actor_delivery(
        false,
        true,
        false,
        false,
        false,
        EvaluationCase::Interrupted,
    );
}

#[test]
fn revocation_during_verifier_suppresses_assessment() {
    exercise_actor_delivery(
        false,
        true,
        false,
        false,
        false,
        EvaluationCase::RevokedDuring,
    );
}

#[test]
fn revocation_during_owner_revalidation_suppresses_success() {
    exercise_actor_delivery(
        false,
        true,
        false,
        false,
        false,
        EvaluationCase::OwnerRevokedDuring,
    );
}

#[test]
fn revocation_during_revalidation_after_cleanup_blocks_delivery() {
    exercise_actor_delivery(
        false,
        true,
        false,
        false,
        false,
        EvaluationCase::RevokedAfterCleanup,
    );
}

#[test]
fn deadline_after_cleanup_blocks_positive_delivery() {
    exercise_actor_delivery(
        false,
        true,
        false,
        false,
        false,
        EvaluationCase::ExpiredAfterCleanup,
    );
}

use evaluation_case::EvaluationCase;

fn exercise_actor_delivery(
    reject: bool,
    matched: bool,
    missing_enrollment: bool,
    replace_token: bool,
    revoke_before_handoff: bool,
    evaluation_case: EvaluationCase,
) {
    use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase};
    use carosign_runtime::operation::actor::Step;
    use carosign_runtime::{OperationActor, OperationMsg};

    let (exchange, peer, mut channel) = setup();
    let mut state = carosign_engine::tests_support::initial();
    state.common.context.operation_id = OperationId::from_bytes([3; 16]).unwrap();
    state.common.context.authenticator.id = AuthenticatorId::from_bytes([7; 16]).unwrap();
    state.common.context.authenticator.device_epoch = DeviceEpoch::FIRST;
    let operation_context = state.common.context.clone();
    let grant = carosign_core::authorization::ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let mut actor = OperationActor::new(state);
    actor
        .bind_verification_scope(&grant, exchange.permitted.clone())
        .unwrap();
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    for (spec, receipt) in [
        (EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        (EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
    ] {
        let effect = *actor
            .pending()
            .iter()
            .find(|effect| effect.spec == spec)
            .unwrap();
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt,
            }))
            .unwrap();
        assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    }
    let registry = crate::operations::OperationRegistry::new();
    // SAFETY: geteuid does not use pointers or change the state of the process.
    let caller = carosign_runtime::ports::CallerContext::from_transport(
        [1; 32],
        unsafe { libc::geteuid() },
        carosign_runtime::ports::TransactionId::from_bytes([4; 16]).unwrap(),
    );
    drop(registry.reserve(actor, grant, &caller).unwrap());
    let id = OperationId::from_bytes([3; 16]).unwrap();
    let owner = carosign_runtime::ports::ResourceOwner {
        operation: id,
        worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
        authenticator: carosign_core::device::AuthenticatorRef {
            id: AuthenticatorId::from_bytes([7; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
    };
    let lease = carosign_runtime::ports::ResourceLease::issued_for_fixture(
        carosign_core::ids::LeaseId::from_bytes([10; 16]).unwrap(),
        owner,
        carosign_core::resource::ResourceSet::new(vec![carosign_core::resource::ResourceClaim {
            id: carosign_core::ids::ResourceId::from_bytes([11; 16]).unwrap(),
            mode: carosign_core::resource::AccessMode::Exclusive,
        }])
        .unwrap(),
    );
    let wrong_lease = carosign_runtime::ports::ResourceLease::issued_for_fixture(
        carosign_core::ids::LeaseId::from_bytes([12; 16]).unwrap(),
        owner,
        carosign_core::resource::ResourceSet::new(vec![carosign_core::resource::ResourceClaim {
            id: carosign_core::ids::ResourceId::from_bytes([11; 16]).unwrap(),
            mode: carosign_core::resource::AccessMode::Exclusive,
        }])
        .unwrap(),
    );
    {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().lease = Some(wrong_lease);
    }
    assert!(matches!(
        registry.prepare_verification_dispatch(id, CommandId::from_bytes([2; 16]).unwrap()),
        Err(crate::operations::ObservationFailure::ContextMismatch)
    ));
    {
        let mut guard = registry.checkout(id).unwrap();
        guard.operation_mut().lease = Some(lease);
        guard.operation_mut().lease_matches_request = true;
    }
    assert!(matches!(
        registry.prepare_verification_dispatch(id, CommandId::from_bytes([2; 16]).unwrap()),
        Ok(Step::Advanced { .. })
    ));
    let frame_context = carosign_adapters::provider::VerificationFrameContext {
        worker: owner.worker,
        command: CommandId::from_bytes([2; 16]).unwrap(),
        operation: id,
        authenticator: owner.authenticator,
        provider_subject: operation_context.subject,
        uid: 1000,
        purpose: operation_context.purpose,
        challenge: carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([9; 16])
            .unwrap(),
        recipient: carosign_evidence::recipient::RecipientRef::new([10; 16]).unwrap(),
    };
    let before_deadline = carosign_core::operation::context::BootInstant::new(
        operation_context.deadline.boot(),
        operation_context.deadline.instant_ns().saturating_sub(1),
    );
    let (frame, permitted) = registry
        .verification_frame(&frame_context, before_deadline)
        .unwrap();
    assert_eq!(frame.operation_id, id.as_bytes());
    assert_eq!(permitted, exchange.permitted);
    let mut foreign = frame_context.clone();
    foreign.command = CommandId::from_bytes([14; 16]).unwrap();
    assert!(matches!(
        registry.verification_frame(&foreign, before_deadline),
        Err(crate::operations::ObservationFailure::ContextMismatch)
    ));
    let deadline = carosign_core::operation::context::BootInstant::new(
        operation_context.deadline.boot(),
        operation_context.deadline.instant_ns(),
    );
    assert!(matches!(
        registry.verification_frame(&frame_context, deadline),
        Err(crate::operations::ObservationFailure::DeadlineExpired)
    ));
    use test_authority::TestAuthority;
    let clock = carosign_adapters::clock::StepClock::new(operation_context.deadline.boot());
    use legacy_authority::LegacyAuthority;
    assert!(matches!(
        futures_executor::block_on(registry.revalidated_verification_frame(
            &frame_context,
            &clock,
            &LegacyAuthority
        )),
        Err(
            crate::operations::VerificationPreparationFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Unavailable
            )
        )
    ));
    let allow = TestAuthority {
        clock: &clock,
        deny: false,
        advance: 0,
    };
    let mut forged = frame_context.clone();
    forged.uid = 0;
    assert!(matches!(
        futures_executor::block_on(
            registry.revalidated_verification_frame(&forged, &clock, &allow)
        ),
        Err(
            crate::operations::VerificationPreparationFailure::Operation(
                crate::operations::ObservationFailure::ContextMismatch
            )
        )
    ));
    assert!(
        futures_executor::block_on(registry.revalidated_verification_frame(
            &frame_context,
            &clock,
            &allow
        ))
        .is_ok()
    );
    let revoke = TestAuthority {
        clock: &clock,
        deny: true,
        advance: 0,
    };
    assert!(matches!(
        futures_executor::block_on(registry.revalidated_verification_frame(
            &frame_context,
            &clock,
            &revoke
        )),
        Err(
            crate::operations::VerificationPreparationFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Revoked
            )
        )
    ));
    let slow = TestAuthority {
        clock: &clock,
        deny: false,
        advance: operation_context.deadline.instant_ns(),
    };
    assert!(matches!(
        futures_executor::block_on(registry.revalidated_verification_frame(
            &frame_context,
            &clock,
            &slow
        )),
        Err(
            crate::operations::VerificationPreparationFailure::Operation(
                crate::operations::ObservationFailure::DeadlineExpired
            )
        )
    ));
    assert!(
        registry
            .prepare_verification_dispatch(id, CommandId::from_bytes([2; 16]).unwrap())
            .is_ok_and(|step| matches!(step, Step::Rejected(_)))
    );
    if reject {
        use crate::verification_owner::{OwnerError, OwnerEvent, VerificationOwner};
        let pump = VerificationPump::new(exchange);
        let context = pump.context().unwrap();
        for foreign in [
            VerificationContext {
                worker: WorkerInstanceId::from_bytes([15; 16]).unwrap(),
                ..context
            },
            VerificationContext {
                command: CommandId::from_bytes([15; 16]).unwrap(),
                ..context
            },
            VerificationContext {
                authenticator: AuthenticatorId::from_bytes([15; 16]).unwrap(),
                ..context
            },
            VerificationContext {
                epoch: DeviceEpoch::new(2).unwrap(),
                ..context
            },
        ] {
            assert!(matches!(
                registry.apply_verification_rejection(&foreign),
                Err(crate::operations::ObservationFailure::ContextMismatch)
            ));
        }
        let mut owner = VerificationOwner::new(id, pump);
        let guard = registry.checkout(id).unwrap();
        send(
            &peer,
            &mut channel,
            3,
            Payload::CommandAck(carosign_provider_sdk::dispatch::refusal(
                common::ErrorCode::ErrorUnavailable,
                "отказ",
            )),
        );
        assert!(matches!(
            owner.next(&registry, Duration::from_secs(1)),
            Err(OwnerError::Pump(PumpError::Delivery(
                crate::operations::ObservationFailure::Busy
            )))
        ));
        drop(guard);
        assert!(matches!(
            owner.next(&registry, Duration::ZERO),
            Ok(OwnerEvent::StepReady)
        ));
        assert!(
            matches!(owner.pending_step(), Some(Step::Advanced { state, effects })
                if state.common.terminal == Some(carosign_engine::TerminalOutcome::Failed)
                && effects.iter().any(|effect| effect.spec == EffectSpec::ReleaseResources)
                && !state.common.pending_effects.iter().any(|effect| effect.spec == EffectSpec::SendVerify))
        );
        let effects = crate::effect_owner::EffectOwner::default();
        owner.handoff(&effects).unwrap();
        assert!(matches!(effects.step(id), Some(Step::Advanced { .. })));
        assert!(registry.has_retained_lease(id));
        return;
    }
    let mut guard = registry.checkout(id).unwrap();
    // The earlier message is processed before the ACK.
    guard
        .operation_mut()
        .actor
        .post(OperationMsg::Domain(DomainInput::Progress))
        .unwrap();
    drop(guard);
    assert!(matches!(
        registry.verification_frame(&frame_context, before_deadline),
        Err(crate::operations::ObservationFailure::AwaitingActor)
    ));
    let guard = registry.checkout(id).unwrap();
    let mut pump = VerificationPump::new(exchange);
    send(
        &peer,
        &mut channel,
        3,
        Payload::CommandAck(provider::CommandAck {
            accepted: true,
            rejection: None,
        }),
    );
    assert!(matches!(
        pump.next(&registry, Duration::from_secs(1)),
        Err(PumpError::Delivery(
            crate::operations::ObservationFailure::Busy
        ))
    ));
    drop(guard);
    assert!(matches!(
        pump.next(&registry, Duration::ZERO),
        Ok(PumpEvent::Accepted(Step::Advanced { .. }))
    ));
    assert_eq!(registry.phase(id), Some(OperationPhase::Running));
    assert!(matches!(
        pump.next(&registry, Duration::ZERO),
        Err(PumpError::Delivery(
            crate::operations::ObservationFailure::AwaitingActor
        ))
    ));
    {
        let mut guard = registry.checkout(id).unwrap();
        let actor = &mut guard.operation_mut().actor;
        assert!(matches!(actor.run_once(), Step::Advanced { .. }));
        let progress = *actor
            .pending()
            .iter()
            .find(|effect| effect.spec == EffectSpec::PublishProgress)
            .unwrap();
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect: progress,
                receipt: EffectReceipt::ProgressPublished,
            }))
            .unwrap();
        assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    }
    let guard = registry.checkout(id).unwrap();
    send(
        &peer,
        &mut channel,
        3,
        Payload::VerificationOutcome(provider::VerificationOutcome {
            status: if matched {
                provider::VerificationStatus::VerifyMatched as i32
            } else {
                provider::VerificationStatus::VerifyNoUsableSample as i32
            },
            evidence: (matched && evaluation_case != EvaluationCase::ScopedAssertion)
                .then(|| provider::verification_outcome::Evidence::OpaqueToken(vec![42; 32])),
            matched_enrollment: (matched && !missing_enrollment).then(|| common::EnrollmentRef {
                enrollment_id: vec![6; 16],
                authenticator_id: vec![7; 16],
                generation: 1,
            }),
            ..Default::default()
        }),
    );
    assert!(matches!(
        pump.next(&registry, Duration::from_secs(1)),
        Err(PumpError::Delivery(
            crate::operations::ObservationFailure::Busy
        ))
    ));
    assert!(matches!(
        pump.next(&registry, Duration::ZERO),
        Err(PumpError::Delivery(
            crate::operations::ObservationFailure::Busy
        ))
    ));
    drop(guard);
    let PumpEvent::Outcome(progress) = pump.next(&registry, Duration::from_secs(1)).unwrap() else {
        panic!("ожидался переданный результат")
    };
    if matched {
        assert!(matches!(
            progress,
            crate::operations::ObservationProgress::Held(
                carosign_runtime::operation::actor::ObservationError::EvidenceRequired
            )
        ));
        let evidence = carosign_core::ids::EvidenceId::from_bytes([90; 16]).unwrap();
        if evaluation_case == EvaluationCase::ScopedAssertion {
            let assertion = registry
                .take_observed_scoped_assertion(id, before_deadline)
                .unwrap();
            assert_eq!(assertion.operation(), id);
            assert!(assertion.matched().is_none());
            assert!(!assertion.permitted().bindings().is_empty());
            assert!(
                registry
                    .take_observed_scoped_assertion(id, before_deadline)
                    .is_err()
            );
            let mut guard = registry.checkout(id).unwrap();
            let actor = &guard.operation_mut().actor;
            assert_eq!(actor.state().common.phase, OperationPhase::Running);
            assert!(actor.state().common.terminal.is_none());
            return;
        }
        if missing_enrollment {
            assert!(
                registry
                    .hold_observed_verification_token(id, evidence, before_deadline)
                    .is_err()
            );
            assert_eq!(registry.phase(id), Some(OperationPhase::Running));
            return;
        }
        if replace_token {
            let mut guard = registry.checkout(id).unwrap();
            let actor = &mut guard.operation_mut().actor;
            let original = actor.take_matched_token().unwrap();
            let store = carosign_evidence::store::EvidenceStore::new(1).unwrap();
            let foreign = store
                .hold(
                    evidence,
                    id,
                    operation_context.deadline,
                    zeroize::Zeroizing::new(vec![43; 32]),
                    before_deadline,
                )
                .unwrap();
            assert!(actor.finish_matched_transfer(&foreign).is_err());
            assert!(!actor.has_work());
            assert_eq!(original.as_slice(), &[42; 32]);
            return;
        }
        let reference = registry
            .hold_observed_verification_token(id, evidence, before_deadline)
            .unwrap();
        assert_eq!(reference.id(), *evidence.as_bytes());
        assert_eq!(reference.operation(), *id.as_bytes());
        assert_eq!(registry.phase(id), Some(OperationPhase::EvaluatingEvidence));
        // In this trial, the command is sent before the registry is built; until its context
        // is fixed, the evaluation area is not issued.
        assert!(
            registry
                .matched_evidence_context(id, &reference, before_deadline)
                .is_err()
        );
        registry.fixture_record_sent_verification(frame_context.clone());
        let context = registry
            .matched_evidence_context(id, &reference, before_deadline)
            .unwrap();
        assert_eq!(
            context.enrollment.id,
            EnrollmentId::from_bytes([6; 16]).unwrap()
        );
        assert_eq!(context.enrollment.generation, EnrollmentGeneration::FIRST);
        assert_eq!(context.subject, operation_context.subject);
        assert_eq!(context.recipient, frame_context.recipient);
        assert_eq!(context.challenge, frame_context.challenge);
        assert!(
            registry
                .matched_evidence_context(id, &reference, deadline)
                .is_err()
        );
        let effects = crate::effect_owner::EffectOwner::default();
        if revoke_before_handoff {
            let guard = registry.checkout(id).unwrap();
            assert!(matches!(
                registry.revoke_verification(id),
                Err(crate::operations::ObservationFailure::Busy)
            ));
            drop(guard);
            assert!(registry.handoff_matched_evidence(id, &effects).is_err());
            assert!(
                registry
                    .take_actor_evidence(id, reference, before_deadline)
                    .is_err()
            );
            return;
        }
        let blocked = crate::effect_owner::EffectOwner::default();
        {
            let mut guard = registry.checkout(id).unwrap();
            let actor = &guard.operation_mut().actor;
            blocked
                .accept_step(
                    id,
                    &Step::Advanced {
                        state: Box::new(actor.state().clone()),
                        effects: actor.pending().to_vec(),
                    },
                )
                .unwrap();
        }
        assert!(registry.handoff_matched_evidence(id, &blocked).is_err());
        registry.handoff_matched_evidence(id, &effects).unwrap();
        assert!(registry.handoff_matched_evidence(id, &effects).is_err());
        assert!(
            matches!(effects.step(id), Some(Step::Advanced { ref effects, .. })
                if effects.iter().any(|effect| effect.spec == EffectSpec::VerifyEvidence))
        );
        use test_software_verifier::TestSoftwareVerifier;
        let effect = match effects.step(id).unwrap() {
            Step::Advanced { effects, .. } => *effects
                .iter()
                .find(|e| e.spec == EffectSpec::VerifyEvidence)
                .unwrap(),
            _ => panic!("нет оценки"),
        };
        effects.acknowledge_step(id).unwrap();
        let evaluation_clock =
            carosign_adapters::clock::StepClock::new(operation_context.deadline.boot());
        let verifier = TestSoftwareVerifier {
            registry: &registry,
            clock: &evaluation_clock,
            wrong_assessment: evaluation_case == EvaluationCase::WrongAssessment,
            interrupted: evaluation_case == EvaluationCase::Interrupted,
            revoke_during: evaluation_case == EvaluationCase::RevokedDuring,
        };
        if evaluation_case == EvaluationCase::Interrupted {
            use std::future::Future;
            let mut work = Box::pin(effects.execute_evidence(
                &registry,
                effect,
                reference.clone(),
                carosign_evidence::requirements::EvidenceRequirements::local_login(),
                &verifier,
                &evaluation_clock,
            ));
            let mut context = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(work.as_mut().poll(&mut context).is_pending());
            drop(work);
            assert!(matches!(
                futures_executor::block_on(effects.execute_evidence(
                    &registry,
                    effect,
                    reference.clone(),
                    carosign_evidence::requirements::EvidenceRequirements::local_login(),
                    &verifier,
                    &evaluation_clock,
                )),
                Err(crate::operations::ObservationFailure::Busy)
            ));
            assert!(
                effects
                    .apply_evidence(&registry, id, &evaluation_clock)
                    .is_err()
            );
            return;
        }
        if evaluation_case == EvaluationCase::Normal {
            let mut owner =
                crate::verification_owner::MatchedEvidenceOwner::new(id, reference.clone(), effect)
                    .unwrap();
            futures_executor::block_on(owner.drive(
                &registry,
                &effects,
                &verifier,
                &carosign_evidence::requirements::EvidenceRequirements::local_login(),
                &evaluation_clock,
            ))
            .unwrap();
            assert!(
                futures_executor::block_on(owner.drive(
                    &registry,
                    &effects,
                    &verifier,
                    &carosign_evidence::requirements::EvidenceRequirements::local_login(),
                    &evaluation_clock,
                ))
                .is_err()
            );
        } else {
            futures_executor::block_on(effects.execute_evidence(
                &registry,
                effect,
                reference.clone(),
                carosign_evidence::requirements::EvidenceRequirements::local_login(),
                &verifier,
                &evaluation_clock,
            ))
            .unwrap();
            if evaluation_case == EvaluationCase::RevokedDuring {
                assert!(
                    effects
                        .apply_evidence(&registry, id, &evaluation_clock)
                        .is_err()
                );
                let mut guard = registry.checkout(id).unwrap();
                assert_ne!(
                    guard.operation_mut().actor.state().common.terminal,
                    Some(carosign_engine::TerminalOutcome::FactorAccepted)
                );
                return;
            }
            if evaluation_case == EvaluationCase::Revoked {
                assert!(matches!(
                    registry.revoke_verification(id),
                    Ok(Step::Advanced { .. })
                ));
                assert!(
                    effects
                        .apply_evidence(&registry, id, &evaluation_clock)
                        .is_err()
                );
                let mut guard = registry.checkout(id).unwrap();
                assert_ne!(
                    guard.operation_mut().actor.state().common.terminal,
                    Some(carosign_engine::TerminalOutcome::FactorAccepted)
                );
                return;
            }
            let guard = registry.checkout(id).unwrap();
            assert!(matches!(
                effects.apply_evidence(&registry, id, &evaluation_clock),
                Err(crate::operations::ObservationFailure::Busy)
            ));
            drop(guard);
            effects
                .apply_evidence(&registry, id, &evaluation_clock)
                .unwrap();
            assert!(registry.handoff_evidence_evaluation(id, &blocked).is_err());
            registry.handoff_evidence_evaluation(id, &effects).unwrap();
            assert!(registry.handoff_evidence_evaluation(id, &effects).is_err());
        }
        if evaluation_case == EvaluationCase::WrongAssessment {
            assert!(
                matches!(effects.step(id), Some(Step::Advanced { state, effects })
                    if state.common.terminal == Some(carosign_engine::TerminalOutcome::Failed)
                    && effects.iter().any(|effect| effect.spec == EffectSpec::DestroyEvidence))
            );
        } else {
            assert!(
                matches!(effects.step(id), Some(Step::Advanced { state, effects })
                    if state.common.terminal.is_none()
                    && effects.iter().any(|effect| effect.spec == EffectSpec::RevalidateOwner))
            );
            use owner_authority::OwnerAuthority;
            let owner_effect = match effects.step(id).unwrap() {
                Step::Advanced { effects, .. } => *effects
                    .iter()
                    .find(|e| e.spec == EffectSpec::RevalidateOwner)
                    .unwrap(),
                _ => panic!("нет перепроверки владельца"),
            };
            effects.acknowledge_step(id).unwrap();
            let denied = OwnerAuthority {
                registry: &registry,
                operation: id,
                revoke: false,
                deny: true,
            };
            assert!(
                futures_executor::block_on(effects.revalidate_evaluated_owner(
                    &registry,
                    id,
                    owner_effect,
                    &denied,
                    &evaluation_clock,
                ))
                .is_err()
            );
            let authority = OwnerAuthority {
                registry: &registry,
                operation: id,
                revoke: evaluation_case == EvaluationCase::OwnerRevokedDuring,
                deny: false,
            };
            let result = futures_executor::block_on(effects.revalidate_evaluated_owner(
                &registry,
                id,
                owner_effect,
                &authority,
                &evaluation_clock,
            ));
            if evaluation_case == EvaluationCase::OwnerRevokedDuring {
                assert!(result.is_err());
                let mut guard = registry.checkout(id).unwrap();
                assert_ne!(
                    guard.operation_mut().actor.state().common.terminal,
                    Some(carosign_engine::TerminalOutcome::FactorAccepted)
                );
                return;
            }
            result.unwrap();
            assert!(registry.handoff_owner_revalidation(id, &blocked).is_err());
            registry.handoff_owner_revalidation(id, &effects).unwrap();
            assert!(
                matches!(effects.step(id), Some(Step::Advanced { state, effects })
                    if state.common.terminal == Some(carosign_engine::TerminalOutcome::FactorAccepted)
                    && effects.iter().any(|effect| effect.spec == EffectSpec::ReleaseResources))
            );
            let cleanup = match effects.step(id).unwrap() {
                Step::Advanced { effects, .. } => *effects
                    .iter()
                    .find(|effect| effect.spec == EffectSpec::ReleaseResources)
                    .unwrap(),
                _ => unreachable!(),
            };
            assert!(
                futures_executor::block_on(registry.revalidate_cleaned_owner(
                    id,
                    owner_effect,
                    &authority,
                    &evaluation_clock,
                ))
                .is_err()
            );
            let set = carosign_core::resource::ResourceSet::new(vec![
                carosign_core::resource::ResourceClaim {
                    id: carosign_core::ids::ResourceId::from_bytes([11; 16]).unwrap(),
                    mode: carosign_core::resource::AccessMode::Exclusive,
                },
            ])
            .unwrap();
            let mut scheduler = carosign_runtime::scheduler::Scheduler::new();
            scheduler.set_epoch(DeviceEpoch::FIRST);
            assert!(matches!(
                scheduler.acquire(
                    carosign_runtime::scheduler::Request {
                        owner,
                        priority: carosign_runtime::scheduler::Priority::SystemAuth,
                        set,
                        since: carosign_runtime::ports::TimeSource::now(&evaluation_clock),
                        mutating: false,
                    },
                    carosign_core::ids::LeaseId::from_bytes([10; 16]).unwrap(),
                ),
                carosign_runtime::scheduler::GrantResult::Granted { .. }
            ));
            let scheduler = std::sync::Mutex::new(scheduler);
            use confirm_cleanup::ConfirmCleanup;
            effects.acknowledge_step(id).unwrap();
            futures_executor::block_on(effects.execute_scheduled_cleanup(
                &registry,
                id,
                cleanup,
                &scheduler,
                &ConfirmCleanup,
                std::future::pending(),
            ))
            .unwrap();
            effects
                .accept_scheduled_cleanup(effects.claim_cleanup(id).unwrap())
                .unwrap();
            let revalidate = match effects.step(id).unwrap() {
                Step::Advanced { effects, .. } => *effects
                    .iter()
                    .find(|effect| effect.spec == EffectSpec::RevalidateOwner)
                    .unwrap(),
                _ => unreachable!(),
            };
            effects.acknowledge_step(id).unwrap();
            if evaluation_case == EvaluationCase::ExpiredAfterCleanup {
                evaluation_clock.advance(operation_context.deadline.instant_ns());
                assert!(
                    futures_executor::block_on(effects.revalidate_cleaned_owner(
                        &registry,
                        id,
                        revalidate,
                        &authority,
                        &evaluation_clock,
                    ))
                    .is_err()
                );
                let mut guard = registry.checkout(id).unwrap();
                assert!(
                    !guard
                        .operation_mut()
                        .actor
                        .pending()
                        .iter()
                        .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
                );
                return;
            }
            assert!(
                futures_executor::block_on(effects.revalidate_cleaned_owner(
                    &registry,
                    id,
                    revalidate,
                    &denied,
                    &evaluation_clock,
                ))
                .is_err()
            );
            if evaluation_case == EvaluationCase::RevokedAfterCleanup {
                let revoke = OwnerAuthority {
                    registry: &registry,
                    operation: id,
                    revoke: true,
                    deny: false,
                };
                assert!(
                    futures_executor::block_on(effects.revalidate_cleaned_owner(
                        &registry,
                        id,
                        revalidate,
                        &revoke,
                        &evaluation_clock,
                    ))
                    .is_err()
                );
                let mut guard = registry.checkout(id).unwrap();
                assert_ne!(
                    guard.operation_mut().actor.state().common.terminal,
                    Some(carosign_engine::TerminalOutcome::FactorAccepted)
                );
                assert!(
                    !guard
                        .operation_mut()
                        .actor
                        .pending()
                        .iter()
                        .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
                );
                return;
            }
            futures_executor::block_on(effects.revalidate_cleaned_owner(
                &registry,
                id,
                revalidate,
                &authority,
                &evaluation_clock,
            ))
            .unwrap();
            assert!(registry.handoff_owner_revalidation(id, &blocked).is_err());
            registry.handoff_owner_revalidation(id, &effects).unwrap();
            assert!(
                matches!(effects.step(id), Some(Step::Advanced { state, effects })
                    if state.common.terminal == Some(carosign_engine::TerminalOutcome::FactorAccepted)
                    && state.common.lease.is_none()
                    && effects.iter().any(|effect| effect.spec == EffectSpec::DeliverOutcome))
            );
            assert!(!registry.has_retained_lease(id));
            // The socket and authority belong exclusively to this trial; the working source
            // of the trusted recipient is not created here.
            use prost::Message;
            let deliver = match effects.step(id).unwrap() {
                Step::Advanced { effects, .. } => *effects
                    .iter()
                    .find(|effect| effect.spec == EffectSpec::DeliverOutcome)
                    .unwrap(),
                _ => unreachable!(),
            };
            effects.acknowledge_step(id).unwrap();
            let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
            let peer = carosign_linux::sys::peer_credentials(&pair.platform).unwrap();
            let credentials = carosign_linux_ipc::credentials::PeerCredentials::from_kernel(
                peer.pid, peer.uid, peer.gid,
            )
            .unwrap();
            let mut session = carosign_linux_ipc::client::session::ClientSession::new(
                credentials,
                *caller.transaction().as_bytes(),
            );
            let hello = carosign_protocol::carosign::client::v1::ClientFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_MAX,
                request_id: vec![1; 16],
                operation_id: Vec::new(),
                event_sequence: 0,
                payload: Some(
                    carosign_protocol::carosign::client::v1::client_frame::Payload::Hello(
                        carosign_protocol::carosign::client::v1::Hello {
                            capture_feedback_v1: false,
                            major: 1,
                            min_minor: carosign_protocol::limits::MINOR_BASELINE,
                            max_minor: carosign_protocol::limits::MINOR_MAX,
                        },
                    ),
                ),
            };
            session.accept(&hello.encode_to_vec()).unwrap();
            let connection = crate::server::Connection {
                session,
                peer,
                fd: pair.platform,
            };
            let mut delivery = crate::verification_owner::PositiveCompletion::new(
                &registry,
                &effects,
                id,
                caller.clone(),
                deliver,
                before_deadline,
            )
            .unwrap();
            assert!(
                futures_executor::block_on(
                    delivery.drive(&connection, &denied, &evaluation_clock,)
                )
                .is_err()
            );
            let progress = futures_executor::block_on(delivery.drive(
                &connection,
                &authority,
                &evaluation_clock,
            ))
            .unwrap();
            assert_eq!(progress, crate::operations::OwnerSendProgress::Sent);
            loop {
                let mut received = [0u8; 4096];
                // SAFETY: The test owns the socket and buffer.
                let count = unsafe {
                    libc::recv(
                        pair.worker.as_raw_fd(),
                        received.as_mut_ptr().cast(),
                        received.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                assert!(count > 0);
                let frame = carosign_protocol::carosign::client::v1::ClientFrame::decode(
                    &received[..count as usize],
                )
                .unwrap();
                assert_eq!(frame.operation_id, id.as_bytes());
                use carosign_protocol::carosign::client::v1::{
                    VerificationResult, client_frame::Payload, terminal_result::Kind,
                };
                let Some(Payload::OperationEvent(event)) = frame.payload else {
                    panic!("ожидалось событие")
                };
                if let Some(terminal) = event.terminal {
                    let Some(Kind::Verification(result)) = terminal.kind else {
                        panic!("ожидался итог проверки")
                    };
                    assert_eq!(
                        result.result,
                        VerificationResult::VerifyFactorAccepted as i32
                    );
                    break;
                }
            }
            assert!(registry.publish_delivered_verification(id).is_err());
            return;
        }
        assert!(
            registry
                .hold_observed_verification_token(id, evidence, before_deadline)
                .is_err()
        );
        let guard = registry.checkout(id).unwrap();
        assert!(matches!(
            registry.revoke_verification(id),
            Err(crate::operations::ObservationFailure::Busy)
        ));
        drop(guard);
        assert!(
            registry
                .hold_observed_verification_token(id, evidence, before_deadline)
                .is_err()
        );
        return;
    }
    assert!(
        matches!(progress, crate::operations::ObservationProgress::Advanced(Step::Advanced { ref state, ref effects })
            if state.common.terminal == Some(carosign_engine::TerminalOutcome::NoSample)
                && effects.iter().any(|effect| effect.spec == EffectSpec::ReleaseResources))
    );
    assert_eq!(registry.phase(id), Some(OperationPhase::Finalizing));
    assert!(pump.into_transport().is_ok());
}

#[test]
fn exchange_does_not_read_for_a_foreign_actor() {
    let (mut exchange, _peer, _channel) = setup();
    let mut actor =
        carosign_runtime::OperationActor::new(carosign_engine::tests_support::initial());
    assert!(matches!(
        exchange.next_for_actor(&mut actor, Duration::ZERO),
        Err(ActorVerificationError::ContextMismatch)
    ));
    assert_eq!(exchange.phase(), VerificationPhase::AwaitingAck);
}
