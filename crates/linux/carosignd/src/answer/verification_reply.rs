use super::{GrantScope, OperationId, SubjectRef, client};

/// The outcome of the operation for the client.
///
/// ⚠️ `Matched` The only thing that gets into the system is what the engine decides after
/// checking the proof, and it's done in a single frame, not in response to the start of the
/// operation.
pub(crate) fn terminal_frame(
    operation: OperationId,
    outcome: carosign_engine::TerminalOutcome,
    sequence: u64,
) -> client::ClientFrame {
    client::ClientFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: Vec::new(),
        operation_id: operation.as_bytes().to_vec(),
        event_sequence: sequence,
        payload: Some(client::client_frame::Payload::OperationEvent(
            client::OperationEvent {
                capture_state: None,
                feedback: None,
                operation_id: operation.as_bytes().to_vec(),
                sequence,
                kind: client::EventKind::EventTerminal as i32,
                phase: client::OperationPhase::OpTerminal as i32,
                percent: 0,
                gap_after_sequence: 0,
                snapshot: None,
                terminal: Some(verification_terminal(outcome)),
            },
        )),
    }
}

pub(crate) fn verification_terminal(
    outcome: carosign_engine::TerminalOutcome,
) -> client::TerminalResult {
    use carosign_engine::TerminalOutcome;
    let result = match outcome {
        TerminalOutcome::FactorAccepted => client::VerificationResult::VerifyFactorAccepted as i32,
        TerminalOutcome::NotMatched => client::VerificationResult::VerifyNotMatched as i32,
        TerminalOutcome::NoSample => client::VerificationResult::VerifyNoSample as i32,
        TerminalOutcome::Cancelled => client::VerificationResult::VerifyCancelled as i32,
        TerminalOutcome::Unavailable => client::VerificationResult::VerifyUnavailable as i32,
        // ⚠️ The rest is rejection, not “almost success.”
        _ => client::VerificationResult::VerifyFailed as i32,
    };
    client::TerminalResult {
        kind: Some(client::terminal_result::Kind::Verification(
            client::VerificationTerminal {
                result,
                error: None,
                matched_enrollment: None,
            },
        )),
    }
}

/// The scope of the request resolution (for inspections).
pub fn scope_of(
    subject: SubjectRef,
    authenticator: carosign_core::ids::AuthenticatorId,
) -> GrantScope {
    GrantScope::Verify {
        subject,
        authenticator,
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
    }
}
