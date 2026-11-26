use carosign_protocol::carosign::client::v1::{
    BeginVerification, EventKind, OperationAccepted, OperationEvent, VerificationResult,
    client_frame::Payload, terminal_result::Kind,
};

use super::super::transport::{Transport, TransportError};
use super::{Observation, VerificationEvents};

/// The maximum time of the client’s observation of one accepted inspection.
pub const VERIFICATION_WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// Performs verification and reports the transaction ID to the cancellation owner.
pub fn observe_verification_with_acceptance(
    transport: &Transport,
    request: BeginVerification,
    accepted_operation: impl FnOnce(&[u8]),
) -> Result<Observation, TransportError> {
    observe_verification_with_progress(
        transport,
        request,
        |accepted| accepted_operation(&accepted.operation_id),
        |_| {},
    )
}

/// Transmits only proven non-terminal events to the UI owner.
pub fn observe_verification_with_progress(
    transport: &Transport,
    request: BeginVerification,
    accepted_operation: impl FnOnce(&OperationAccepted),
    mut progress: impl FnMut(EventKind),
) -> Result<Observation, TransportError> {
    observe_verification_with_events(transport, request, accepted_operation, |event| {
        if let Some(kind) = EventKind::from_i32(event.kind) {
            progress(kind);
        }
    })
}

pub fn observe_verification_with_events(
    transport: &Transport,
    request: BeginVerification,
    accepted_operation: impl FnOnce(&OperationAccepted),
    progress: impl FnMut(&OperationEvent),
) -> Result<Observation, TransportError> {
    observe_verification_with_result(transport, request, accepted_operation, progress)
        .map(|(outcome, _)| outcome)
}

pub fn observe_verification_with_result(
    transport: &Transport,
    request: BeginVerification,
    accepted_operation: impl FnOnce(&OperationAccepted),
    mut progress: impl FnMut(&OperationEvent),
) -> Result<
    (
        Observation,
        Option<carosign_protocol::carosign::common::v1::EnrollmentRef>,
    ),
    TransportError,
> {
    let authenticator_id = request.authenticator_id.clone();
    let selected = request.enrollment.clone();
    let disclosure_requested = request.report_matched_enrollment;
    let response = transport.request(Payload::BeginVerification(request))?;
    let Some(Payload::OperationAccepted(accepted)) = response.payload else {
        return Err(TransportError::Unavailable);
    };
    let mut tracker = VerificationEvents::new(&accepted).map_err(|_| TransportError::Unexpected)?;
    accepted_operation(&accepted);
    let deadline = std::time::Instant::now() + VERIFICATION_WAIT_LIMIT;
    for _ in 0..4096 {
        if std::time::Instant::now() >= deadline {
            return Err(TransportError::Unavailable);
        }
        let (frame, terminal) =
            transport.receive_verification_event_until(&mut tracker, deadline)?;
        if !terminal {
            if let Some(Payload::OperationEvent(event)) = &frame.payload {
                progress(event);
            }
            continue;
        }
        let Some(Payload::OperationEvent(event)) = frame.payload else {
            return Err(TransportError::Unexpected);
        };
        let Some(Kind::Verification(result)) = event.terminal.and_then(|value| value.kind) else {
            return Err(TransportError::Unexpected);
        };
        let outcome = match VerificationResult::from_i32(result.result) {
            Some(VerificationResult::VerifyFactorAccepted) => Observation::FactorAccepted,
            Some(VerificationResult::VerifyNotMatched) => Observation::NotMatched,
            Some(VerificationResult::VerifyNoSample) => Observation::NoSample,
            Some(VerificationResult::VerifyCancelled) => Observation::Cancelled,
            Some(VerificationResult::VerifyUnavailable) => Observation::Unavailable,
            Some(VerificationResult::VerifyFailed) => Observation::Failed,
            _ => return Err(TransportError::Unexpected),
        };
        if let Some(reference) = &result.matched_enrollment
            && (!disclosure_requested
                || outcome != Observation::FactorAccepted
                || reference.authenticator_id != authenticator_id
                || reference.generation == 0
                || reference.enrollment_id.len() != 16
                || reference.enrollment_id.iter().all(|byte| *byte == 0)
                || selected
                    .as_ref()
                    .is_some_and(|selected| selected != reference))
        {
            return Err(TransportError::Unexpected);
        }
        return Ok((outcome, result.matched_enrollment));
    }
    Err(TransportError::Unavailable)
}
