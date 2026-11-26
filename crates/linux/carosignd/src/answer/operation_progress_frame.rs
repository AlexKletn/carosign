use super::{OperationId, client};

pub(crate) fn operation_progress_frame(
    operation: OperationId,
    event: &carosign_runtime::operation::events::Event,
    capture_feedback_v1: bool,
) -> Option<client::ClientFrame> {
    use carosign_engine::OperationPhase as P;
    use carosign_runtime::operation::events::EventKind as K;
    if event.sequence == 0 || event.phase == P::Terminal {
        return None;
    }
    let phase = super::public_phase(event.phase);
    let (kind, percent, feedback) = match &event.kind {
        K::StateChanged => (client::EventKind::EventStateChanged, 0, None),
        K::Progress(progress) => super::capture_feedback::encode_progress(*progress),
        K::Terminal => return None,
    };
    let capture_state = match event.kind {
        K::Progress(carosign_runtime::operation::events::Progress::CaptureState(state)) => {
            Some(state)
        }
        _ => None,
    };
    Some(client::ClientFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: Vec::new(),
        operation_id: operation.as_bytes().to_vec(),
        event_sequence: event.sequence,
        payload: Some(client::client_frame::Payload::OperationEvent(
            client::OperationEvent {
                capture_state,
                feedback: feedback.filter(|_| capture_feedback_v1),
                operation_id: operation.as_bytes().to_vec(),
                sequence: event.sequence,
                kind: kind as i32,
                phase: phase as i32,
                percent,
                gap_after_sequence: 0,
                snapshot: None,
                terminal: None,
            },
        )),
    })
}
