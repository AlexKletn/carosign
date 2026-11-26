use super::super::*;
use crate::operation::events::{CapturePercent, Progress};
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{CommandId, MutationId, OperationId, WorkerInstanceId};

#[test]
fn scoped_assertion_preserves_the_exact_permitted_set_without_token_or_enrollment() {
    use crate::operation::bindings::{BindingKind, EnrollmentBinding, PermittedSet};
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::ids::{AuthenticatorId, EnrollmentId};
    use carosign_provider_sdk::events::{Event, VerificationStatus};

    let worker = WorkerInstanceId::from_bytes([1; 16]).unwrap();
    let operation = OperationId::from_bytes([2; 16]).unwrap();
    let command = CommandId::from_bytes([3; 16]).unwrap();
    let epoch = DeviceEpoch::new(7).unwrap();
    let authenticator = AuthenticatorId::from_bytes([4; 16]).unwrap();
    let permitted = PermittedSet::new(
        vec![5, 6]
            .into_iter()
            .map(|id| EnrollmentBinding {
                enrollment: EnrollmentRef {
                    id: EnrollmentId::from_bytes([id; 16]).unwrap(),
                    authenticator,
                    generation: EnrollmentGeneration::FIRST,
                },
                kind: BindingKind::ProviderOpaque,
                provider_ref: vec![id],
                format_id: None,
                format_version: 0,
            })
            .collect(),
    )
    .unwrap();
    let mut session = Session {
        worker: Some(worker),
        device_epoch: Some(epoch),
        operation: Some(operation),
        accepted: Some(command),
        ..Session::default()
    };
    let event = Event::Verification {
        worker: worker.as_bytes().to_vec(),
        device_epoch: epoch.value(),
        command: command.as_bytes().to_vec(),
        operation: operation.as_bytes().to_vec(),
        status: VerificationStatus::VerifyMatched,
        evidence: None,
        format_id: String::new(),
        matched_enrollment: None,
        error: None,
    };
    let token_event = match event.clone() {
        Event::Verification {
            worker,
            device_epoch,
            command,
            operation,
            status,
            format_id,
            matched_enrollment,
            error,
            ..
        } => Event::Verification {
            worker,
            device_epoch,
            command,
            operation,
            status,
            format_id,
            matched_enrollment,
            error,
            evidence: Some(carosign_provider_sdk::events::ProviderEvidence::OpaqueToken(vec![42])),
        },
        _ => unreachable!(),
    };
    let mut token_session = session.clone();
    let token_observation = token_session
        .accept_verification_outcome(token_event, &permitted)
        .unwrap();
    assert!(token_observation.into_scoped_assertion().is_err());
    let assertion = session
        .accept_verification_outcome(event, &permitted)
        .unwrap()
        .into_scoped_assertion()
        .unwrap();
    assert_eq!(assertion.worker(), worker);
    assert_eq!(assertion.epoch(), epoch);
    assert_eq!(assertion.command(), command);
    assert_eq!(assertion.operation(), operation);
    assert_eq!(assertion.permitted(), &permitted);
    assert_eq!(assertion.matched(), None);
    let context = crate::ports::ScopedAssertionContext {
        subject: carosign_core::identity::SubjectRef {
            authority: carosign_core::ids::AuthorityId::from_bytes([8; 16]).unwrap(),
            subject: carosign_core::ids::SubjectId::from_bytes([9; 16]).unwrap(),
        },
        authenticator: carosign_core::device::AuthenticatorRef {
            id: authenticator,
            device_epoch: epoch,
        },
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
        recipient: carosign_evidence::recipient::RecipientRef::new([10; 16]).unwrap(),
        challenge: carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([11; 16])
            .unwrap(),
        deadline: carosign_core::operation::context::BootDeadline::new(
            carosign_core::ids::BootId::from_bytes([12; 16]).unwrap(),
            50,
        ),
    };
    let record = assertion.into_evidence_record(&context).unwrap();
    assert_eq!(record.operation, operation);
    assert_eq!(record.worker, worker);
    assert_eq!(record.command, command);
    assert_eq!(
        record.permitted,
        permitted
            .bindings()
            .iter()
            .map(|binding| binding.enrollment)
            .collect::<Vec<_>>()
    );
    assert_eq!(record.matched, None);
    assert_eq!(record.recipient, context.recipient);
    assert_eq!(record.challenge, context.challenge);
    assert!(session.terminal);
}

#[test]
fn enrollment_outcome_requires_full_session_and_exact_planned_echo() {
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::ids::{AuthenticatorId, EnrollmentId};
    use carosign_protocol::carosign::{common::v1 as common, provider::v1 as wire};
    use carosign_provider_sdk::events::Event;
    let worker = WorkerInstanceId::from_bytes([1; 16]).unwrap();
    let operation = OperationId::from_bytes([2; 16]).unwrap();
    let command = CommandId::from_bytes([3; 16]).unwrap();
    let mutation = MutationId::from_bytes([4; 16]).unwrap();
    let planned = EnrollmentRef {
        id: EnrollmentId::from_bytes([5; 16]).unwrap(),
        authenticator: AuthenticatorId::from_bytes([6; 16]).unwrap(),
        generation: EnrollmentGeneration::FIRST,
    };
    let echo = common::EnrollmentRef {
        enrollment_id: planned.id.as_bytes().to_vec(),
        authenticator_id: planned.authenticator.as_bytes().to_vec(),
        generation: planned.generation.value(),
    };
    let event = Event::Mutation {
        worker: worker.as_bytes().to_vec(),
        device_epoch: 1,
        command: command.as_bytes().to_vec(),
        operation: operation.as_bytes().to_vec(),
        mutation: mutation.as_bytes().to_vec(),
        status: wire::MutationStatus::MutationProviderCompleted,
        recovery_ref: vec![8],
        provider_ref: vec![9],
        enrollment: Some(echo),
    };
    let mut session = Session {
        worker: Some(worker),
        device_epoch: Some(DeviceEpoch::FIRST),
        operation: Some(operation),
        accepted: Some(command),
        ..Session::default()
    };
    for invalid in [
        {
            let mut value = event.clone();
            if let Event::Mutation { worker, .. } = &mut value {
                *worker = vec![7; 16];
            }
            value
        },
        {
            let mut value = event.clone();
            if let Event::Mutation { device_epoch, .. } = &mut value {
                *device_epoch = 2;
            }
            value
        },
        {
            let mut value = event.clone();
            if let Event::Mutation { mutation, .. } = &mut value {
                *mutation = vec![7; 16];
            }
            value
        },
        {
            let mut value = event.clone();
            if let Event::Mutation { enrollment, .. } = &mut value {
                *enrollment = None;
            }
            value
        },
        {
            let mut value = event.clone();
            if let Event::Mutation {
                enrollment: Some(echo),
                ..
            } = &mut value
            {
                echo.generation += 1;
            }
            value
        },
        {
            let mut value = event.clone();
            if let Event::Mutation { provider_ref, .. } = &mut value {
                *provider_ref = vec![1; carosign_core::enrollment::OPAQUE_REFERENCE_MAX + 1];
            }
            value
        },
    ] {
        assert!(
            session
                .accept_enrollment_outcome(invalid, mutation, planned)
                .is_err()
        );
        assert!(!session.terminal);
    }
    let checked = session
        .accept_enrollment_outcome(event.clone(), mutation, planned)
        .unwrap();
    assert_eq!(checked.event(), &event);
    let mut context = carosign_engine::tests_support::initial().common.context;
    context.operation_id = operation;
    context.authenticator.id = planned.authenticator;
    let subject = context.subject;
    let request = carosign_core::operation::mutation::EnrollRequest {
        subject,
        authenticator: planned.authenticator,
        target: carosign_core::enrollment::EnrollmentLabel::new("палец").unwrap(),
        replace: None,
    };
    let mut state = carosign_engine::OperationState::enrollment(context, request, mutation);
    state.common.phase = carosign_engine::OperationPhase::InFlight;
    state.common.worker = Some(carosign_engine::transition::WorkerRef {
        instance: worker,
        device_epoch: DeviceEpoch::FIRST,
    });
    state.common.command = Some(carosign_engine::transition::CommandRef {
        id: command,
        transition_index: carosign_engine::transition::TransitionIndex::new(1),
    });
    state.body.mutation_mut().unwrap().planned_new = Some(planned);
    for mismatch in 0..4 {
        let mut other = state.clone();
        match mismatch {
            0 => other.common.worker = None,
            1 => other.common.command = None,
            2 => other.common.context.authenticator.device_epoch = DeviceEpoch::new(2).unwrap(),
            _ => {
                other.common.context.authenticator.id =
                    AuthenticatorId::from_bytes([8; 16]).unwrap()
            }
        }
        let mut source = session.clone();
        source.terminal = false;
        let observation = source
            .accept_enrollment_outcome(event.clone(), mutation, planned)
            .unwrap();
        let mut other_actor = crate::operation::actor::OperationActor::new(other);
        assert_eq!(
            other_actor.hold_enrollment_observation(observation),
            Err(crate::operation::actor::ObservationError::ContextMismatch)
        );
    }
    let mut actor = crate::operation::actor::OperationActor::new(state);
    assert_eq!(actor.hold_enrollment_observation(checked), Ok(()));
    assert_eq!(actor.enrollment_observation().unwrap().event(), &event);
    assert_eq!(actor.run_once(), crate::operation::actor::Step::Idle);
    assert_eq!(
        session.accept_enrollment_outcome(event, mutation, planned),
        Err(SessionError::SecondTerminal)
    );
}

#[test]
fn parsed_progress_keeps_payload_after_channel_and_session_validation() {
    use carosign_protocol::carosign::provider::v1 as wire;
    use carosign_provider_sdk::{events, wire::Channel};
    use prost::Message;

    let frame = |frame_seq, operation_sequence, percent| wire::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_MAX,
        worker_instance_id: vec![1; 16],
        frame_seq,
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        uid: None,
        payload: Some(wire::provider_frame::Payload::ProviderProgress(
            wire::ProviderProgress {
                capture_state: None,
                feedback: None,
                operation_sequence,
                kind: wire::ProgressKind::ProgressCapturing as i32,
                percent,
            },
        )),
    };
    let mut channel = Channel::new();
    let mut session = Session {
        worker: Some(WorkerInstanceId::from_bytes([1; 16]).unwrap()),
        device_epoch: Some(DeviceEpoch::FIRST),
        operation: Some(OperationId::from_bytes([3; 16]).unwrap()),
        accepted: Some(CommandId::from_bytes([2; 16]).unwrap()),
        ..Session::default()
    };
    let first = channel.accept(&frame(1, 1, 45).encode_to_vec()).unwrap();
    let event = events::parse(&first).unwrap();
    let validated = session.accept_progress_event(event).unwrap();
    assert_eq!(
        validated.operation,
        OperationId::from_bytes([3; 16]).unwrap()
    );
    assert_eq!(validated.provider_sequence, 1);
    assert_eq!(
        validated.progress,
        Progress::Capturing(CapturePercent::new(45).unwrap())
    );

    let duplicate = channel.accept(&frame(2, 1, 45).encode_to_vec()).unwrap();
    assert_eq!(
        session.accept_progress_event(events::parse(&duplicate).unwrap()),
        Err(SessionError::ProgressOutOfOrder)
    );
    assert_eq!(session.last_progress_sequence, Some(1));
    assert!(channel.accept(&frame(3, 2, 101).encode_to_vec()).is_err());
    assert_eq!(
        channel.accept(&frame(3, 2, 45).encode_to_vec()),
        Err(carosign_provider_sdk::wire::TransportError::Closed)
    );
}

#[test]
fn progress_variants_preserve_semantics_and_reject_forged_payloads() {
    use carosign_provider_sdk::events::{Event, ProgressKind};
    let mut session = Session {
        worker: Some(WorkerInstanceId::from_bytes([1; 16]).unwrap()),
        device_epoch: Some(DeviceEpoch::FIRST),
        operation: Some(OperationId::from_bytes([3; 16]).unwrap()),
        accepted: Some(CommandId::from_bytes([2; 16]).unwrap()),
        ..Session::default()
    };
    let event = |sequence, kind, percent| Event::Progress {
        capture_state: None,
        feedback: None,
        worker: vec![1; 16],
        device_epoch: 1,
        command: vec![2; 16],
        operation: vec![3; 16],
        sequence,
        kind,
        percent,
    };
    for (index, (kind, expected)) in [
        (ProgressKind::ProgressPlaceFinger, Progress::PlaceFinger),
        (ProgressKind::ProgressMoveFinger, Progress::MoveFinger),
        (ProgressKind::ProgressRemoveFinger, Progress::RemoveFinger),
        (ProgressKind::ProgressRetry, Progress::Retry),
        (ProgressKind::ProgressWorking, Progress::Working),
    ]
    .into_iter()
    .enumerate()
    {
        let observation = session
            .accept_progress_event(event(index as u64 + 1, kind, 0))
            .unwrap();
        assert_eq!(observation.progress, expected);
    }
    for (kind, percent) in [
        (ProgressKind::ProgressWorking, 1),
        (ProgressKind::ProgressCapturing, 101),
        (ProgressKind::ProgressUnspecified, 0),
    ] {
        assert_eq!(
            session.accept_progress_event(event(6, kind, percent)),
            Err(SessionError::MalformedProgress)
        );
        assert_eq!(session.last_progress_sequence, Some(5));
    }
}

#[test]
fn accepted_or_completed_session_is_not_free() {
    let mut session = Session::default();
    assert!(session.free());
    session.accept(CommandId::from_bytes([1; 16]).unwrap());
    assert!(!session.free());
    session.terminal().unwrap();
    assert!(!session.free());
}

#[test]
fn reserved_operation_is_not_free_before_acceptance() {
    let session = Session {
        operation: Some(OperationId::from_bytes([1; 16]).unwrap()),
        ..Session::default()
    };
    assert!(!session.free());
}

#[test]
fn repeated_acceptance_never_replaces_the_original_command() {
    let mut session = Session::default();
    let first = CommandId::from_bytes([1; 16]).unwrap();
    let second = CommandId::from_bytes([2; 16]).unwrap();
    session.accept(first);
    session.accept(second);
    assert_eq!(session.accepted, Some(first));
    session.terminal().unwrap();
    session.accept(second);
    assert_eq!(session.accepted, Some(first));
}

#[test]
fn progress_checks_fences_order_and_terminal_without_mutating_on_error() {
    let worker = WorkerInstanceId::from_bytes([1; 16]).unwrap();
    let stranger = WorkerInstanceId::from_bytes([2; 16]).unwrap();
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let other_operation = OperationId::from_bytes([4; 16]).unwrap();
    let command = CommandId::from_bytes([5; 16]).unwrap();
    let epoch = DeviceEpoch::FIRST;
    let mut session = Session {
        worker: Some(worker),
        device_epoch: Some(epoch),
        operation: Some(operation),
        ..Session::default()
    };
    assert_eq!(
        session.accept_progress(worker, epoch, operation, command, 1),
        Err(SessionError::NotAccepted)
    );
    session.accept(command);
    let unchanged = session.clone();
    for (w, e, op) in [
        (stranger, epoch, operation),
        (worker, epoch.next().unwrap(), operation),
        (worker, epoch, other_operation),
    ] {
        assert_eq!(
            session.accept_progress(w, e, op, command, 1),
            Err(SessionError::StaleProgress)
        );
        assert_eq!(session, unchanged);
    }
    assert_eq!(
        session.accept_progress(
            worker,
            epoch,
            operation,
            CommandId::from_bytes([6; 16]).unwrap(),
            1
        ),
        Err(SessionError::WrongCommand)
    );
    assert_eq!(
        session.accept_progress(worker, epoch, operation, command, 2),
        Err(SessionError::ProgressOutOfOrder)
    );
    assert_eq!(session, unchanged);
    session
        .accept_progress(worker, epoch, operation, command, 1)
        .unwrap();
    session
        .accept_progress(worker, epoch, operation, command, 3)
        .unwrap();
    let unchanged = session.clone();
    for sequence in [0, 1, 3] {
        assert_eq!(
            session.accept_progress(worker, epoch, operation, command, sequence),
            Err(SessionError::ProgressOutOfOrder)
        );
        assert_eq!(session, unchanged);
    }
    session.terminal().unwrap();
    assert_eq!(
        session.accept_progress(worker, epoch, operation, command, 4),
        Err(SessionError::ProgressAfterTerminal)
    );
}
