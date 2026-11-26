use super::*;
use carosign_core::ids::SubjectId;
use carosign_protocol::carosign::client::v1::client_frame::Payload;

fn subject() -> SubjectRef {
    SubjectRef {
        authority: carosign_core::ids::AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

#[test]
fn accepted_carries_operation_and_deadline() {
    let reply = Reply::Accepted {
        operation: OperationId::from_bytes([3u8; 16]).expect("валид"),
        boot: BootId::from_bytes([4u8; 16]).expect("валид"),
        instant_ns: 1_000,
    };
    let frame = reply.frame(&[5u8; 16]);
    match frame.payload {
        Some(Payload::OperationAccepted(accepted)) => {
            assert_eq!(accepted.operation_id, vec![3u8; 16]);
            let deadline = accepted.deadline.expect("срок");
            assert_eq!(deadline.boot_id, vec![4u8; 16]);
            assert_eq!(deadline.boottime_ns, 1_000);
        }
        other => panic!("ожидалось принятие, а пришло {other:?}"),
    }
}

#[test]
fn refusals_have_different_codes() {
    // ⚠️ Three troubles, three wire codes.
    assert_eq!(
        error_of(&AcceptError::NotEnrolled).code,
        common::ErrorCode::ErrorSubjectUnknown as i32
    );
    assert_eq!(
        error_of(&AcceptError::Unauthorized(
            carosign_runtime::ports::AuthorityRefusal::Denied
        ))
        .code,
        common::ErrorCode::ErrorUnauthorized as i32
    );
    assert_eq!(
        error_of(&AcceptError::IdentityUnavailable).code,
        common::ErrorCode::ErrorUnavailable as i32
    );
}

#[test]
fn scope_is_verify() {
    assert!(matches!(
        scope_of(
            subject(),
            carosign_core::ids::AuthenticatorId::from_bytes([3; 16]).unwrap()
        ),
        GrantScope::Verify { .. }
    ));
}

#[test]
fn completed_mutation_snapshot_preserves_the_durable_outcome() {
    use carosign_engine::{OperationPhase, TerminalOutcome};
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let mut snapshot = carosign_runtime::operation::events::OperationSnapshot {
        operation,
        phase: OperationPhase::Terminal,
        last_sequence: 129,
        terminal: Some(TerminalOutcome::NoChange),
    };
    let wire = completed_mutation_snapshot(&snapshot).unwrap();
    assert_eq!(wire.operation_id, operation.as_bytes());
    assert_eq!(wire.last_event_sequence, 129);
    assert!(
        matches!(wire.terminal.unwrap().kind, Some(client::terminal_result::Kind::Mutation(value)) if value.result == client::MutationResult::MutationNoChange as i32)
    );
    snapshot.terminal = Some(TerminalOutcome::FactorAccepted);
    assert!(completed_mutation_snapshot(&snapshot).is_none());
    snapshot.terminal = Some(TerminalOutcome::MutationCommitted);
    snapshot.phase = OperationPhase::Finalizing;
    assert!(completed_mutation_snapshot(&snapshot).is_none());
}

#[test]
fn mutation_event_preserves_phase_progress_and_terminal_result() {
    use carosign_engine::{OperationPhase, TerminalOutcome};
    use carosign_runtime::operation::events::{CapturePercent, Event, EventKind, Progress};
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let progress = Event {
        sequence: 7,
        phase: OperationPhase::InFlight,
        kind: EventKind::Progress(Progress::Capturing(CapturePercent::new(42).unwrap())),
    };
    let frame = completed_mutation_event(operation, &progress, TerminalOutcome::NoChange).unwrap();
    assert_eq!(frame.event_sequence, 7);
    let Some(Payload::OperationEvent(event)) = frame.payload else {
        panic!("нет события")
    };
    assert_eq!(event.sequence, 7);
    assert_eq!(event.phase, client::OperationPhase::OpRunning as i32);
    assert_eq!(event.kind, client::EventKind::EventProgress as i32);
    assert_eq!(event.percent, 42);
    assert!(event.terminal.is_none());
    let terminal = Event {
        sequence: 8,
        phase: OperationPhase::Terminal,
        kind: EventKind::Terminal,
    };
    let frame = completed_mutation_event(operation, &terminal, TerminalOutcome::NoChange).unwrap();
    let Some(Payload::OperationEvent(event)) = frame.payload else {
        panic!("нет события")
    };
    assert!(
        matches!(event.terminal.unwrap().kind, Some(client::terminal_result::Kind::Mutation(value)) if value.result == client::MutationResult::MutationNoChange as i32)
    );
    assert!(
        completed_mutation_event(operation, &terminal, TerminalOutcome::FactorAccepted).is_none()
    );
    let invalid = Event {
        phase: OperationPhase::Running,
        ..terminal
    };
    assert!(completed_mutation_event(operation, &invalid, TerminalOutcome::NoChange).is_none());
}

#[test]
fn completed_replay_encodes_all_events_or_refuses_inconsistent_history() {
    use carosign_adapters::mutation::AuthorizedMutationReplay;
    use carosign_engine::{OperationPhase, TerminalOutcome};
    use carosign_runtime::operation::events::{Event, EventKind, Replay};
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let event = Event {
        sequence: 10,
        phase: OperationPhase::Running,
        kind: EventKind::StateChanged,
    };
    let terminal = Event {
        sequence: 11,
        phase: OperationPhase::Terminal,
        kind: EventKind::Terminal,
    };
    let mut replay = AuthorizedMutationReplay {
        operation,
        outcome: TerminalOutcome::NoChange,
        replay: Replay::Events(vec![event.clone(), terminal.clone()]),
    };
    let frames = completed_mutation_replay_frames(&replay).unwrap();
    assert_eq!(
        frames
            .iter()
            .map(|frame| frame.event_sequence)
            .collect::<Vec<_>>(),
        vec![10, 11]
    );
    replay.replay = Replay::Events(vec![
        event.clone(),
        Event {
            sequence: 12,
            ..terminal.clone()
        },
    ]);
    assert_eq!(
        completed_mutation_replay_frames(&replay),
        Err(ReplayEncodingError::InvalidHistory)
    );
    replay.replay = Replay::Events(vec![terminal, event]);
    assert_eq!(
        completed_mutation_replay_frames(&replay),
        Err(ReplayEncodingError::InvalidHistory)
    );
}

#[test]
fn completed_replay_gap_is_not_silently_converted_to_empty_history() {
    use carosign_adapters::mutation::AuthorizedMutationReplay;
    use carosign_engine::{OperationPhase, TerminalOutcome};
    use carosign_runtime::operation::events::{OperationSnapshot, Replay};

    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let replay = AuthorizedMutationReplay {
        operation,
        outcome: TerminalOutcome::NoChange,
        replay: Replay::Gap {
            after_sequence: 0,
            last_sequence: u64::MAX,
            snapshot: OperationSnapshot {
                operation,
                phase: OperationPhase::Terminal,
                last_sequence: u64::MAX,
                terminal: Some(TerminalOutcome::NoChange),
            },
        },
    };
    assert_eq!(
        completed_mutation_replay_frames(&replay),
        Err(ReplayEncodingError::GapNeedsSequenceContract)
    );
}

#[test]
fn provider_retry_crosses_wire_and_keeps_its_payload() {
    use carosign_core::{
        device::DeviceEpoch,
        ids::{CommandId, WorkerInstanceId},
    };
    use carosign_engine::OperationPhase;
    use carosign_protocol::carosign::{
        common::v1::{
            CaptureAction, CaptureFeedback, CapturePrompt, CaptureRetry, CaptureRetryReason,
            CaptureTarget, capture_feedback::Kind,
        },
        provider::v1::{
            ProviderFrame, ProviderProgress, provider_frame::Payload as ProviderPayload,
        },
    };
    use carosign_runtime::{
        operation::events::{EventKind, EventStream},
        provider::session::Session,
    };
    use prost::Message;

    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let worker = WorkerInstanceId::from_bytes([4; 16]).unwrap();
    let command = CommandId::from_bytes([5; 16]).unwrap();
    let epoch = DeviceEpoch::new(7).unwrap();
    let feedback = CaptureFeedback {
        kind: Some(Kind::CaptureRetry(CaptureRetry {
            reason: Some(CaptureRetryReason::PartialCapture as i32),
            prompt: Some(CapturePrompt {
                action: CaptureAction::Center as i32,
                target: CaptureTarget::Finger as i32,
            }),
        })),
    };
    let provider = ProviderFrame {
        major: 1,
        minor: 4,
        frame_seq: 10,
        worker_instance_id: worker.as_bytes().to_vec(),
        device_epoch: epoch.value(),
        operation_id: operation.as_bytes().to_vec(),
        command_id: command.as_bytes().to_vec(),
        payload: Some(ProviderPayload::ProviderProgress(ProviderProgress {
            operation_sequence: 1,
            feedback: Some(feedback),
            ..Default::default()
        })),
        ..Default::default()
    };
    let provider = ProviderFrame::decode(provider.encode_to_vec().as_slice()).unwrap();
    let mut session = Session {
        worker: Some(worker),
        device_epoch: Some(epoch),
        operation: Some(operation),
        accepted: Some(command),
        ..Default::default()
    };
    let parse = || carosign_provider_sdk::events::parse(&provider).unwrap();
    let observation = session.accept_progress_event(parse()).unwrap();
    assert!(session.accept_progress_event(parse()).is_err());

    let mut stream = EventStream::new(operation);
    stream
        .publish(
            OperationPhase::Running,
            EventKind::Progress(observation.progress),
        )
        .unwrap();
    let frame = operation_progress_frame(operation, stream.latest().unwrap(), true).unwrap();
    let decoded = client::ClientFrame::decode(frame.encode_to_vec().as_slice()).unwrap();
    let Some(Payload::OperationEvent(event)) = decoded.payload else {
        panic!("expected operation event");
    };
    assert_eq!(event.operation_id, operation.as_bytes());
    assert_eq!(event.sequence, 1);
    assert_eq!(event.kind, client::EventKind::EventRetry as i32);
    assert_eq!(event.feedback, Some(feedback));
    assert!(event.terminal.is_none());
}
