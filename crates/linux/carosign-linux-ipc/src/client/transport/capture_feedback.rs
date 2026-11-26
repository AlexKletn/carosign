use carosign_protocol::{
    carosign::{
        client::v1::{EventKind, OperationEvent},
        common::v1::{CaptureFeedback, CaptureRetryReason, capture_feedback::Kind},
    },
    validation::check_capture_feedback,
};

use super::TransportError;

pub(super) fn normalize(
    event: &mut OperationEvent,
    negotiated: bool,
) -> Result<(), TransportError> {
    if event.capture_state.is_some_and(|state| {
        event.kind() != EventKind::EventWorking
            || event.terminal.is_some()
            || event.feedback.is_some()
            || !carosign_protocol::validation::valid_capture_state(&state)
    }) {
        return Err(TransportError::Unexpected);
    }
    let Some(mut feedback) = event.feedback.take() else {
        return Ok(());
    };
    if !negotiated
        || !matches!(
            EventKind::from_i32(event.kind),
            Some(
                EventKind::EventPlaceFinger
                    | EventKind::EventMoveFinger
                    | EventKind::EventRemoveFinger
                    | EventKind::EventRetry
                    | EventKind::EventWorking
                    | EventKind::EventProgress
            )
        )
        || event.terminal.is_some()
    {
        return Err(TransportError::Unexpected);
    }
    if let Some(Kind::CaptureRetry(retry)) = &mut feedback.kind {
        retry.reason = retry.reason.filter(|reason| {
            CaptureRetryReason::from_i32(*reason)
                .is_some_and(|reason| reason != CaptureRetryReason::Unspecified)
        });
        retry.prompt = retry.prompt.filter(|prompt| {
            check_capture_feedback(&CaptureFeedback {
                kind: Some(Kind::Prompt(*prompt)),
            })
        });
    }
    match feedback.kind {
        Some(Kind::Percent(_) | Kind::AcceptedSamples(_)) if !check_capture_feedback(&feedback) => {
            return Err(TransportError::Unexpected);
        }
        _ => {}
    }
    event.feedback = Some(feedback).filter(check_capture_feedback);
    Ok(())
}

#[path = "capture_feedback.tests.rs"]
#[cfg(test)]
mod tests;
