use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::Command;

use carosign_protocol::carosign::common::v1 as common;
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_provider_sdk::wire::Channel;

fn receive(fd: &OwnedFd) -> Vec<u8> {
    let mut buffer = vec![0; 65536];
    // SAFETY: The buffer and descriptor remain valid throughout the call.
    let size = unsafe { libc::recv(fd.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len(), 0) };
    assert!(size > 0);
    buffer.truncate(size as usize);
    buffer
}

fn exercise_actor_exchange(
    host: OwnedFd,
    channel: Channel,
    mut frame: provider::ProviderFrame,
    permitted: carosign_runtime::PermittedSet,
    scenario: &str,
) -> (OwnedFd, Channel) {
    use carosign_core::device::DeviceEpoch;
    use carosign_core::ids::{AuthenticatorId, CommandId, OperationId, WorkerInstanceId};
    use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
    use carosign_runtime::operation::actor::{ObservationError, Step};
    use carosign_runtime::{OperationActor, OperationMsg};
    use carosignd::provider_verification::{ActorVerificationMessage, VerificationExchange};
    use std::time::Duration;

    frame.command_id = vec![70; 16];
    frame.operation_id = vec![71; 16];
    let mut state = carosign_engine::tests_support::initial();
    state.common.context.operation_id = OperationId::from_bytes([71; 16]).unwrap();
    state.common.context.authenticator.id = AuthenticatorId::from_bytes([6; 16]).unwrap();
    state.common.context.authenticator.device_epoch = DeviceEpoch::FIRST;
    let grant = carosign_core::authorization::ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let mut actor = OperationActor::new(state);
    actor
        .bind_verification_scope(&grant, permitted.clone())
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
    let effect = *actor
        .pending()
        .iter()
        .find(|effect| effect.spec == EffectSpec::SendVerify)
        .unwrap();
    actor
        .post(OperationMsg::Domain(
            DomainInput::BindVerificationDispatch {
                effect,
                lease: carosign_core::ids::LeaseId::from_bytes([7; 16]).unwrap(),
                worker: carosign_engine::WorkerRef {
                    instance: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
                    device_epoch: DeviceEpoch::FIRST,
                },
                command: CommandId::from_bytes([70; 16]).unwrap(),
            },
        ))
        .unwrap();
    assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    let transport =
        carosign_linux_ipc::provider::transport::ProviderTransport::new(host, channel).unwrap();
    let mut exchange =
        VerificationExchange::start(transport, frame, permitted, Duration::from_secs(5)).unwrap();
    assert!(matches!(
        exchange.next_for_actor(&mut actor, Duration::from_secs(5)),
        Ok(ActorVerificationMessage::Accepted)
    ));
    assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    let delivered = exchange
        .next_for_actor(&mut actor, Duration::from_secs(5))
        .unwrap();
    if scenario.starts_with("matched") {
        assert!(matches!(
            delivered,
            ActorVerificationMessage::OutcomeHeld(ObservationError::EvidenceRequired)
        ));
        assert!(actor.verification_observation().is_some());
        assert!(actor.state().common.terminal.is_none());
    } else {
        assert!(matches!(delivered, ActorVerificationMessage::OutcomeQueued));
        assert!(matches!(actor.run_once(), Step::Advanced { .. }));
        assert_eq!(
            actor.state().common.terminal,
            Some(carosign_engine::TerminalOutcome::NoSample)
        );
        assert!(actor.expects(EffectSpec::ReleaseResources));
    }
    exchange.into_transport().unwrap().into_parts().unwrap()
}

fn exercise_scenario(scenario: &str, fault: &str) {
    let mut pair = [0; 2];
    // SAFETY: The array contains two returnable descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, pair.as_mut_ptr()) },
        0
    );
    // SAFETY: Both descriptors are obtained by socketpair and are transmitted to exactly one
    // owner.
    let (host, worker) = unsafe { (OwnedFd::from_raw_fd(pair[0]), OwnedFd::from_raw_fd(pair[1])) };
    // SAFETY: host is an open descriptor; the child process does not need it.
    assert_eq!(
        unsafe { libc::fcntl(host.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
        0
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_carosign-mock-provider"))
        .arg(worker.as_raw_fd().to_string())
        .arg(scenario)
        .spawn()
        .unwrap();
    drop(worker);
    let mut channel = Channel::new();
    let hello = channel.accept(&receive(&host)).unwrap();
    assert!(
        matches!(hello.payload, Some(Payload::WorkerHello(ref h)) if h.min_minor == carosign_protocol::limits::MINOR_BASELINE && h.max_minor == carosign_protocol::limits::MINOR_MAX)
    );
    let instance = vec![1; 16];
    let initialize = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance.clone(),
        frame_seq: 0,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::Initialize(provider::Initialize {
            worker_instance_id: instance.clone(),
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: vec![2; 16],
            profile_id: vec![3; 16],
            enabled_features: vec![provider::Feature::BasicOperations as i32],
        })),
    };
    let bytes = channel.prepare(initialize).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    let initialized = channel.accept(&receive(&host)).unwrap();
    assert!(
        matches!(initialized.payload, Some(Payload::Initialized(ref i)) if i.ready_for_binding)
    );
    let authenticator = common::AuthenticatorRef {
        authenticator_id: vec![6; 16],
        device_epoch: 1,
    };
    let mut bind = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance.clone(),
        frame_seq: 0,
        command_id: vec![4; 16],
        operation_id: Vec::new(),
        device_epoch: 1,
        uid: None,
        payload: Some(Payload::BindAuthenticator(provider::BindAuthenticator {
            authenticator: Some(authenticator.clone()),
            profile_id: vec![3; 16],
            lease_id: vec![7; 16],
            fd_slots: Vec::new(),
        })),
    };
    if fault == "foreign-profile"
        && let Some(Payload::BindAuthenticator(value)) = &mut bind.payload
    {
        value.profile_id = vec![33; 16];
    }
    let bytes = channel.prepare(bind).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    if fault == "foreign-profile" {
        let mut byte = [0; 1];
        // SAFETY: The descriptor and buffer remain valid throughout the read call.
        assert_eq!(
            unsafe { libc::recv(host.as_raw_fd(), byte.as_mut_ptr().cast(), byte.len(), 0) },
            0
        );
        assert!(!child.wait().unwrap().success());
        return;
    }
    let ready = channel.accept(&receive(&host)).unwrap();
    assert!(
        matches!(ready.payload, Some(Payload::AuthenticatorReady(ref ready)) if ready.authenticator.as_ref() == Some(&authenticator) && ready.capabilities.as_ref().unwrap().operations == vec![common::ProviderOperation::ProviderVerify as i32, common::ProviderOperation::ProviderEnroll as i32, common::ProviderOperation::ProviderRemove as i32])
    );
    let describe = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance.clone(),
        frame_seq: 0,
        command_id: vec![8; 16],
        operation_id: Vec::new(),
        device_epoch: 1,
        uid: None,
        payload: Some(Payload::Describe(provider::Describe {
            authenticator: Some(authenticator.clone()),
        })),
    };
    let bytes = channel.prepare(describe).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    let described = channel.accept(&receive(&host)).unwrap();
    assert!(matches!(described.payload, Some(Payload::Described(_))));
    let seed = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance.clone(),
        frame_seq: 0,
        command_id: vec![23; 16],
        operation_id: vec![24; 16],
        device_epoch: 1,
        uid: Some(1000),
        payload: Some(Payload::StartEnrollment(provider::StartEnrollment {
            authenticator: Some(authenticator.clone()),
            subject: Some(common::SubjectRef {
                authority_id: vec![11; 16],
                subject_id: vec![12; 16],
            }),
            mutation_id: vec![25; 16],
            label: "test".into(),
            replace: None,
            replace_binding: None,
            enrollment_authorization_ref: Vec::new(),
            enrollment_challenge_digest: Vec::new(),
            planned_enrollment: Some(common::EnrollmentRef {
                enrollment_id: vec![15; 16],
                authenticator_id: authenticator.authenticator_id.clone(),
                generation: 1,
            }),
        })),
    };
    let bytes = channel.prepare(seed).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    assert!(
        matches!(channel.accept(&receive(&host)).unwrap().payload, Some(Payload::CommandAck(ref ack)) if ack.accepted)
    );
    assert!(matches!(
        channel.accept(&receive(&host)).unwrap().payload,
        Some(Payload::MutationOutcome(_))
    ));
    let (verify, permitted) = {
        use carosign_adapters::provider::{VerificationFrameContext, verification_frame};
        use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
        use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
        use carosign_core::identity::SubjectRef;
        use carosign_core::ids::{
            AuthenticatorId, AuthorityId, CommandId, EnrollmentId, OperationId, SubjectId,
            WorkerInstanceId,
        };
        use carosign_core::operation::context::VerificationPurpose;
        use carosign_evidence::recipient::{ChallengeRef, RecipientRef};
        use carosign_runtime::operation::bindings::{
            BindingKind, EnrollmentBinding, PermittedSet,
        };
        let authenticator_id =
            AuthenticatorId::try_from(authenticator.authenticator_id.as_slice()).unwrap();
        let context = VerificationFrameContext {
            uid: 1000,
            worker: WorkerInstanceId::try_from(instance.as_slice()).unwrap(),
            command: CommandId::from_bytes([9; 16]).unwrap(),
            operation: OperationId::from_bytes([10; 16]).unwrap(),
            authenticator: AuthenticatorRef {
                id: authenticator_id,
                device_epoch: DeviceEpoch::FIRST,
            },
            provider_subject: SubjectRef {
                authority: AuthorityId::from_bytes([11; 16]).unwrap(),
                subject: SubjectId::from_bytes([12; 16]).unwrap(),
            },
            purpose: VerificationPurpose::SessionLogin,
            challenge: ChallengeRef::accepted_by_recipient([13; 16]).unwrap(),
            recipient: RecipientRef::new([14; 16]).unwrap(),
        };
        let permitted = PermittedSet::new(vec![EnrollmentBinding {
            enrollment: EnrollmentRef {
                id: EnrollmentId::from_bytes([15; 16]).unwrap(),
                authenticator: authenticator_id,
                generation: EnrollmentGeneration::FIRST,
            },
            kind: BindingKind::ProviderOpaque,
            provider_ref: vec![15; 16],
            format_id: None,
            format_version: 0,
        }])
        .unwrap();
        (verification_frame(&context, &permitted).unwrap(), permitted)
    };
    if scenario == "pending-cancel" {
        let bytes = channel.prepare(verify).unwrap();
        // SAFETY: The descriptor and buffer remain valid throughout the send call.
        assert_eq!(
            unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
            bytes.len() as isize
        );
        assert!(
            matches!(channel.accept(&receive(&host)).unwrap().payload, Some(Payload::CommandAck(ref ack)) if ack.accepted)
        );
        let cancel = provider::ProviderFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_BASELINE,
            worker_instance_id: instance.clone(),
            frame_seq: 0,
            command_id: vec![26; 16],
            operation_id: vec![10; 16],
            device_epoch: 1,
            uid: None,
            payload: Some(Payload::RequestCancel(provider::RequestCancel {
                target_operation_id: vec![10; 16],
            })),
        };
        let bytes = channel.prepare(cancel).unwrap();
        // SAFETY: The descriptor and buffer remain valid throughout the send call.
        assert_eq!(
            unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
            bytes.len() as isize
        );
        let receipt = channel.accept(&receive(&host)).unwrap();
        assert!(
            matches!(receipt.payload, Some(Payload::CancelReceipt(ref result)) if result.status == provider::CancelStatus::CancelRequested as i32)
        );
        let outcome = channel.accept(&receive(&host)).unwrap();
        assert_eq!(outcome.command_id, vec![9; 16]);
        assert!(
            matches!(outcome.payload, Some(Payload::VerificationOutcome(ref result)) if result.status == provider::VerificationStatus::VerifyCancelled as i32 && result.evidence.is_none())
        );
        drop(host);
        assert!(child.wait().unwrap().success());
        return;
    }
    use carosignd::provider_verification::{VerificationExchange, VerificationMessage};
    let transport =
        carosign_linux_ipc::provider::transport::ProviderTransport::new(host, channel).unwrap();
    let mut exchange = VerificationExchange::start(
        transport,
        verify.clone(),
        permitted.clone(),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    assert!(matches!(
        exchange.next(std::time::Duration::from_secs(5)).unwrap(),
        VerificationMessage::Accepted
    ));
    let VerificationMessage::Outcome {
        observation: checked,
    } = exchange.next(std::time::Duration::from_secs(5)).unwrap()
    else {
        panic!("ожидался исход проверки")
    };
    let checked = *checked;
    let outcome = checked.event().clone();
    let transport = exchange.into_transport().unwrap();
    let (host, channel) = transport.into_parts().unwrap();
    assert!(
        matches!(&outcome, carosign_provider_sdk::events::Event::Verification { operation, .. } if operation == &vec![10; 16])
    );
    let (host, mut channel) =
        exercise_actor_exchange(host, channel, verify.clone(), permitted, scenario);
    {
        use carosign_core::device::DeviceEpoch;
        use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
        use carosign_core::ids::{
            AuthenticatorId, CommandId, EnrollmentId, OperationId, WorkerInstanceId,
        };
        use carosign_runtime::operation::bindings::{
            BindingKind, EnrollmentBinding, PermittedSet,
        };
        use carosign_runtime::provider::session::{Session, SessionError};
        let allowed = PermittedSet::new(vec![EnrollmentBinding {
            enrollment: EnrollmentRef {
                id: EnrollmentId::from_bytes([15; 16]).unwrap(),
                authenticator: AuthenticatorId::from_bytes([6; 16]).unwrap(),
                generation: EnrollmentGeneration::FIRST,
            },
            kind: BindingKind::ProviderOpaque,
            provider_ref: vec![15; 16],
            format_id: None,
            format_version: 0,
        }])
        .unwrap();
        let mut session = Session {
            worker: Some(WorkerInstanceId::from_bytes([1; 16]).unwrap()),
            device_epoch: Some(DeviceEpoch::FIRST),
            operation: Some(OperationId::from_bytes([10; 16]).unwrap()),
            accepted: Some(CommandId::from_bytes([9; 16]).unwrap()),
            ..Session::default()
        };
        let event = outcome.clone();
        let mut stale = event.clone();
        if let carosign_provider_sdk::events::Event::Verification { device_epoch, .. } = &mut stale
        {
            *device_epoch = 2;
        }
        assert!(
            session
                .accept_verification_outcome(stale, &allowed)
                .is_err()
        );
        if scenario.starts_with("matched") {
            let mut foreign = event.clone();
            if let carosign_provider_sdk::events::Event::Verification {
                matched_enrollment: Some(reference),
                ..
            } = &mut foreign
            {
                reference.generation += 1;
            }
            assert!(
                session
                    .accept_verification_outcome(foreign, &allowed)
                    .is_err()
            );
        }
        let mut revoked_session = session.clone();
        let mut mismatched_session = session.clone();
        let mut mismatched_binding = allowed.bindings()[0].clone();
        mismatched_binding.provider_ref = vec![99; 16];
        let mismatched_set = PermittedSet::new(vec![mismatched_binding]).unwrap();
        let mismatched = mismatched_session
            .accept_verification_outcome(event.clone(), &mismatched_set)
            .unwrap();
        let _checked_again = session
            .accept_verification_outcome(event.clone(), &allowed)
            .unwrap();
        let mut state = carosign_engine::tests_support::initial();
        state.common.context.operation_id = OperationId::from_bytes([10; 16]).unwrap();
        state.common.context.authenticator.id = AuthenticatorId::from_bytes([6; 16]).unwrap();
        state.common.context.authenticator.device_epoch = DeviceEpoch::FIRST;
        state.common.worker = Some(carosign_engine::transition::WorkerRef {
            instance: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        });
        state.common.command = Some(carosign_engine::transition::CommandRef {
            id: CommandId::from_bytes([9; 16]).unwrap(),
            transition_index: carosign_engine::TransitionIndex::new(1),
        });
        let grant = carosign_core::authorization::ValidatedGrant::issue_for_port(
            state.common.context.owner.binding,
            state.common.context.owner.authority,
            state.required_grant_scope(),
            state.common.context.deadline.boot(),
            state.common.context.deadline.instant_ns(),
        );
        let running_actor = || {
            use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
            use carosign_runtime::OperationMsg;
            let mut actor = carosign_runtime::OperationActor::new(state.clone());
            actor
                .bind_verification_scope(&grant, allowed.clone())
                .unwrap();
            actor
                .post(OperationMsg::Domain(DomainInput::Authorized))
                .unwrap();
            assert!(matches!(
                actor.run_once(),
                carosign_runtime::operation::actor::Step::Advanced { .. }
            ));
            for (spec, receipt) in [
                (EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
                (EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
                (EffectSpec::SendVerify, EffectReceipt::SendAccepted),
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
                assert!(matches!(
                    actor.run_once(),
                    carosign_runtime::operation::actor::Step::Advanced { .. }
                ));
            }
            assert_eq!(
                actor.state().common.phase,
                carosign_engine::OperationPhase::Running
            );
            actor
        };
        for (status, expected) in [
            (
                provider::VerificationStatus::VerifyNotMatched,
                carosign_engine::TerminalOutcome::NotMatched,
            ),
            (
                provider::VerificationStatus::VerifyCancelled,
                carosign_engine::TerminalOutcome::Cancelled,
            ),
            (
                provider::VerificationStatus::VerifyFailed,
                carosign_engine::TerminalOutcome::Failed,
            ),
        ] {
            let mut alternate = event.clone();
            if let carosign_provider_sdk::events::Event::Verification {
                status: value,
                matched_enrollment,
                evidence,
                ..
            } = &mut alternate
            {
                *value = status;
                *matched_enrollment = None;
                *evidence = None;
            }
            let mut source = revoked_session.clone();
            let observation = source
                .accept_verification_outcome(alternate, &allowed)
                .unwrap();
            let mut target = running_actor();
            target.hold_verification_observation(observation).unwrap();
            target.advance_verification_observation().unwrap();
            assert!(matches!(
                target.run_once(),
                carosign_runtime::operation::actor::Step::Advanced { .. }
            ));
            assert_eq!(target.state().common.terminal, Some(expected));
            assert_eq!(
                target.state().common.phase,
                carosign_engine::OperationPhase::Finalizing
            );
            assert!(target.expects(carosign_engine::EffectSpec::ReleaseResources));
        }
        {
            let mut detailed = event.clone();
            if let carosign_provider_sdk::events::Event::Verification {
                status,
                matched_enrollment,
                evidence,
                error,
                ..
            } = &mut detailed
            {
                *status = provider::VerificationStatus::VerifyFailed;
                *matched_enrollment = None;
                *evidence = None;
                *error = Some(common::Error {
                    code: common::ErrorCode::ErrorUnavailable as i32,
                    safe_message: "unavailable".into(),
                });
            }
            let mut source = revoked_session.clone();
            let observation = source
                .accept_verification_outcome(detailed, &allowed)
                .unwrap();
            let mut target = running_actor();
            target.hold_verification_observation(observation).unwrap();
            assert_eq!(target.advance_verification_observation(), Ok(()));
            assert!(target.verification_observation().is_some());
            assert!(matches!(
                target.run_once(),
                carosign_runtime::operation::actor::Step::Advanced { .. }
            ));
            assert_eq!(target.state().common.terminal, Some(carosign_engine::TerminalOutcome::Failed));
            assert_eq!(target.state().common.phase, carosign_engine::OperationPhase::Finalizing);
            assert!(target.expects(carosign_engine::EffectSpec::ReleaseResources));
        }
        {
            let mut source = revoked_session.clone();
            let mut no_sample = event.clone();
            if let carosign_provider_sdk::events::Event::Verification {
                status,
                matched_enrollment,
                evidence,
                ..
            } = &mut no_sample
            {
                *status = provider::VerificationStatus::VerifyNoUsableSample;
                *matched_enrollment = None;
                *evidence = None;
            }
            let observation = source
                .accept_verification_outcome(no_sample, &allowed)
                .unwrap();
            let mut target = running_actor();
            target.hold_verification_observation(observation).unwrap();
            for _ in 0..carosign_runtime::operation::mailbox::NORMAL_CAPACITY {
                target
                    .post(carosign_runtime::OperationMsg::Domain(
                        carosign_engine::DomainInput::Progress,
                    ))
                    .unwrap();
            }
            assert_eq!(
                target.advance_verification_observation(),
                Err(carosign_runtime::operation::actor::ObservationError::MailboxFull)
            );
            assert!(target.verification_observation().is_some());
            target
                .post(carosign_runtime::OperationMsg::GrantRevoked)
                .unwrap();
            assert!(target.verification_observation().is_none());
            assert_eq!(
                target.advance_verification_observation(),
                Err(carosign_runtime::operation::actor::ObservationError::ContextMismatch)
            );
        }
        let revoked = revoked_session
            .accept_verification_outcome(event.clone(), &allowed)
            .unwrap();
        let mut revoked_actor = running_actor();
        revoked_actor
            .post(carosign_runtime::OperationMsg::GrantRevoked)
            .unwrap();
        assert_eq!(
            revoked_actor.hold_verification_observation(revoked),
            Err(carosign_runtime::operation::actor::ObservationError::ContextMismatch)
        );
        assert!(revoked_actor.verification_observation().is_none());
        let mut actor = running_actor();
        assert_eq!(
            actor.hold_verification_observation(mismatched),
            Err(carosign_runtime::operation::actor::ObservationError::ContextMismatch)
        );
        assert!(actor.verification_observation().is_none());
        assert_eq!(actor.hold_verification_observation(checked), Ok(()));
        assert!(actor.verification_observation().is_some());
        if scenario.starts_with("matched") {
            assert_eq!(
                actor.advance_verification_observation(),
                Err(carosign_runtime::operation::actor::ObservationError::EvidenceRequired)
            );
            assert!(actor.verification_observation().is_some());
            actor
                .post(carosign_runtime::OperationMsg::GrantRevoked)
                .unwrap();
            assert!(actor.verification_observation().is_none());
        } else {
            assert_eq!(actor.advance_verification_observation(), Ok(()));
            assert!(actor.verification_observation().is_none());
            assert!(matches!(
                actor.run_once(),
                carosign_runtime::operation::actor::Step::Advanced { .. }
            ));
            assert_eq!(
                actor.state().common.phase,
                carosign_engine::OperationPhase::Finalizing
            );
            assert_eq!(
                actor.state().common.terminal,
                Some(carosign_engine::TerminalOutcome::NoSample)
            );
            assert!(actor.expects(carosign_engine::EffectSpec::ReleaseResources));
        }
        assert_eq!(
            session.accept_verification_outcome(event, &allowed),
            Err(SessionError::SecondTerminal)
        );
    }
    match outcome {
        carosign_provider_sdk::events::Event::Verification {
            status,
            evidence,
            matched_enrollment,
            ..
        } => {
            if scenario == "matched-token" {
                assert!(
                    matches!(evidence, Some(carosign_provider_sdk::events::ProviderEvidence::OpaqueToken(ref token)) if token.as_slice() == [42; 32])
                );
            } else {
                assert!(evidence.is_none());
            }
            if scenario.starts_with("matched") {
                assert_eq!(status, provider::VerificationStatus::VerifyMatched);
                assert_eq!(matched_enrollment.unwrap().enrollment_id, vec![15; 16]);
            } else {
                assert_eq!(status, provider::VerificationStatus::VerifyNoUsableSample);
                assert!(matched_enrollment.is_none());
            }
        }
        other => panic!("неверный исход: {other:?}"),
    }
    for (command, operation, expected) in [
        (26, 10, provider::CancelStatus::CancelAlreadyTerminal),
        (27, 28, provider::CancelStatus::CancelUnsupported),
    ] {
        let cancel = provider::ProviderFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_BASELINE,
            worker_instance_id: instance.clone(),
            frame_seq: 0,
            command_id: vec![command; 16],
            operation_id: vec![operation; 16],
            device_epoch: 1,
            uid: None,
            payload: Some(Payload::RequestCancel(provider::RequestCancel {
                target_operation_id: vec![operation; 16],
            })),
        };
        let bytes = channel.prepare(cancel).unwrap();
        // SAFETY: The descriptor and buffer remain valid throughout the send call.
        assert_eq!(
            unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
            bytes.len() as isize
        );
        let receipt = channel.accept(&receive(&host)).unwrap();
        assert_eq!(receipt.command_id, vec![command; 16]);
        assert!(
            matches!(receipt.payload, Some(Payload::CancelReceipt(ref receipt)) if receipt.status == expected as i32)
        );
    }
    let subject = common::SubjectRef {
        authority_id: vec![11; 16],
        subject_id: vec![12; 16],
    };
    let mut previous: Option<common::EnrollmentRef> = None;
    for id in [20u8, 21] {
        let planned = common::EnrollmentRef {
            enrollment_id: vec![id; 16],
            authenticator_id: authenticator.authenticator_id.clone(),
            generation: 1,
        };
        let binding = previous.as_ref().map(|old| provider::EnrollmentBinding {
            enrollment: Some(old.clone()),
            kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
            provider_opaque_ref: old.enrollment_id.clone(),
            format_id: String::new(),
            format_version: 0,
        });
        let enroll = provider::ProviderFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_BASELINE,
            worker_instance_id: instance.clone(),
            frame_seq: 0,
            command_id: vec![id; 16],
            operation_id: vec![id + 10; 16],
            device_epoch: 1,
            uid: Some(1000),
            payload: Some(Payload::StartEnrollment(provider::StartEnrollment {
                authenticator: Some(authenticator.clone()),
                subject: Some(subject.clone()),
                mutation_id: vec![id + 20; 16],
                label: "test".into(),
                replace: previous.clone(),
                replace_binding: binding,
                enrollment_authorization_ref: Vec::new(),
                enrollment_challenge_digest: Vec::new(),
                planned_enrollment: Some(planned.clone()),
            })),
        };
        let bytes = channel.prepare(enroll).unwrap();
        // SAFETY: The descriptor and buffer remain valid throughout the send call.
        assert_eq!(
            unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
            bytes.len() as isize
        );
        let ack = channel.accept(&receive(&host)).unwrap();
        assert!(matches!(ack.payload, Some(Payload::CommandAck(ref ack)) if ack.accepted));
        let result = channel.accept(&receive(&host)).unwrap();
        assert!(
            matches!(result.payload, Some(Payload::MutationOutcome(ref result)) if result.status == provider::MutationStatus::MutationProviderCompleted as i32 && result.enrollment.as_ref() == Some(&planned) && result.provider_opaque_ref == planned.enrollment_id)
        );
        previous = Some(planned);
    }
    let target = previous.unwrap();
    let remove = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance.clone(),
        frame_seq: 0,
        command_id: vec![22; 16],
        operation_id: vec![32; 16],
        device_epoch: 1,
        uid: Some(1000),
        payload: Some(Payload::StartRemoval(provider::StartRemoval {
            authenticator: Some(authenticator.clone()),
            subject: Some(subject),
            mutation_id: vec![42; 16],
            enrollment: Some(target.clone()),
            provider_opaque_ref: target.enrollment_id.clone(),
            enrollment_binding: Some(provider::EnrollmentBinding {
                enrollment: Some(target.clone()),
                kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: target.enrollment_id.clone(),
                format_id: String::new(),
                format_version: 0,
            }),
        })),
    };
    let bytes = channel.prepare(remove).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    let ack = channel.accept(&receive(&host)).unwrap();
    assert!(matches!(ack.payload, Some(Payload::CommandAck(ref ack)) if ack.accepted));
    let result = channel.accept(&receive(&host)).unwrap();
    assert!(
        matches!(result.payload, Some(Payload::MutationOutcome(ref result)) if result.status == provider::MutationStatus::MutationProviderCompleted as i32 && result.enrollment.as_ref() == Some(&target))
    );
    let unsupported = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance,
        frame_seq: 0,
        command_id: vec![17; 16],
        operation_id: vec![5; 16],
        device_epoch: 1,
        uid: None,
        payload: Some(Payload::QueryMutation(provider::QueryMutation {
            authenticator: Some(authenticator.clone()),
            mutation_id: vec![19; 16],
            recovery_ref: Vec::new(),
        })),
    };
    let bytes = channel.prepare(unsupported).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    let ack = channel.accept(&receive(&host)).unwrap();
    assert_eq!(ack.command_id, vec![17; 16]);
    assert!(
        matches!(ack.payload, Some(Payload::CommandAck(ref ack)) if ack.accepted && ack.rejection.is_none())
    );
    let result = channel.accept(&receive(&host)).unwrap();
    assert_eq!(result.command_id, vec![17; 16]);
    assert!(carosign_linux_ipc::provider::bridge::observe(&result).is_err());
    assert!(
        matches!(result.payload, Some(Payload::QueryMutationResult(ref result)) if result.mutation_id == vec![19; 16] && result.state == provider::QueryState::QueryUnsupported as i32)
    );
    let bytes = match fault {
        // Same one. CommandId new frame_seq It should not give rise to a second outcome.
        "duplicate-command" => channel.prepare(verify).unwrap(),
        "completed-operation" => {
            let mut repeated = verify;
            repeated.command_id = vec![18; 16];
            channel.prepare(repeated).unwrap()
        }
        "rebind" => {
            let mut repeated = verify;
            repeated.uid = None;
            repeated.command_id = vec![18; 16];
            repeated.operation_id.clear();
            repeated.payload = Some(Payload::BindAuthenticator(provider::BindAuthenticator {
                authenticator: Some(authenticator.clone()),
                profile_id: vec![3; 16],
                lease_id: vec![7; 16],
                fd_slots: Vec::new(),
            }));
            channel.prepare(repeated).unwrap()
        }
        "completed-mutation" => {
            let mut repeated = verify;
            repeated.command_id = vec![18; 16];
            repeated.operation_id = vec![60; 16];
            repeated.payload = Some(Payload::StartEnrollment(provider::StartEnrollment {
                authenticator: Some(authenticator.clone()),
                subject: Some(common::SubjectRef {
                    authority_id: vec![11; 16],
                    subject_id: vec![12; 16],
                }),
                mutation_id: vec![25; 16],
                planned_enrollment: Some(common::EnrollmentRef {
                    enrollment_id: vec![61; 16],
                    authenticator_id: authenticator.authenticator_id.clone(),
                    generation: 1,
                }),
                ..Default::default()
            }));
            channel.prepare(repeated).unwrap()
        }
        "stale-epoch" => {
            let mut stale = verify;
            stale.command_id = vec![18; 16];
            stale.device_epoch = 2;
            channel.prepare(stale).unwrap()
        }
        "removed-enrollment" => {
            let mut stale = verify;
            stale.command_id = vec![18; 16];
            stale.operation_id = vec![60; 16];
            if let Some(Payload::StartVerification(ref mut start)) = stale.payload {
                start.permitted_enrollments[0]
                    .enrollment
                    .as_mut()
                    .unwrap()
                    .enrollment_id = vec![21; 16];
                start.permitted_enrollments[0].provider_opaque_ref = vec![21; 16];
            }
            channel.prepare(stale).unwrap()
        }
        // A repeat of the already received frame checks the number of this direction.
        "duplicate-sequence" => bytes,
        _ => panic!("неизвестное испытание"),
    };
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    let mut buffer = [0; 1];
    // SAFETY: The descriptor and buffer remain valid throughout the read call.
    assert_eq!(
        unsafe {
            libc::recv(
                host.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                0,
            )
        },
        0
    );
    assert!(!child.wait().unwrap().success());
}

#[test]
fn separate_process_exercises_both_verification_scenarios() {
    exercise_scenario("no-sample", "duplicate-command");
    exercise_scenario("matched", "duplicate-command");
    exercise_scenario("matched-token", "duplicate-command");
}

#[test]
fn separate_process_cancels_pending_verification() {
    exercise_scenario("pending-cancel", "");
}

#[test]
fn separate_process_rejects_stale_epoch_and_replayed_frame() {
    exercise_scenario("no-sample", "stale-epoch");
    exercise_scenario("no-sample", "duplicate-sequence");
    exercise_scenario("matched", "removed-enrollment");
    exercise_scenario("no-sample", "completed-operation");
    exercise_scenario("no-sample", "completed-mutation");
}

#[test]
fn separate_process_rejects_foreign_profile_and_rebinding() {
    exercise_scenario("no-sample", "foreign-profile");
    exercise_scenario("no-sample", "rebind");
}

#[test]
fn process_rejects_initialize_with_unadvertised_features() {
    let mut pair = [0; 2];
    // SAFETY: The array has room for two descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, pair.as_mut_ptr()) },
        0
    );
    // SAFETY: Both descriptors have sole owners.
    let (host, worker) = unsafe { (OwnedFd::from_raw_fd(pair[0]), OwnedFd::from_raw_fd(pair[1])) };
    // SAFETY: Host is an open descriptor.
    assert_eq!(
        unsafe { libc::fcntl(host.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
        0
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_carosign-mock-provider"))
        .arg(worker.as_raw_fd().to_string())
        .spawn()
        .unwrap();
    drop(worker);
    let mut channel = Channel::new();
    channel.accept(&receive(&host)).unwrap();
    let init = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::Initialize(provider::Initialize {
            worker_instance_id: vec![1; 16],
            selected_minor: carosign_protocol::limits::MINOR_BASELINE,
            package_id: vec![2; 16],
            profile_id: vec![3; 16],
            enabled_features: vec![provider::Feature::SafeMutationQuery as i32],
        })),
    };
    let bytes = channel.prepare(init).unwrap();
    // SAFETY: The descriptor and buffer remain valid throughout the send call.
    assert_eq!(
        unsafe { libc::send(host.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) },
        bytes.len() as isize
    );
    let mut byte = [0; 1];
    // SAFETY: The descriptor and buffer remain valid throughout the read call.
    assert_eq!(
        unsafe { libc::recv(host.as_raw_fd(), byte.as_mut_ptr().cast(), byte.len(), 0) },
        0
    );
    assert!(!child.wait().unwrap().success());
}
