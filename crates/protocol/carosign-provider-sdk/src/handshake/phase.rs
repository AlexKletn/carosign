use super::HandshakeStep;

use core::fmt;

use carosign_protocol::carosign::provider::v1 as provider;

/// Where we are.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    /// Prepare to send. `WorkerHello`.
    Starting,
    /// We agree on the version and features.
    Negotiating,
    /// Wait. `BindAuthenticator`.
    Binding,
    /// Ready for commands.
    Ready,
    /// Let's go.
    Draining,
    /// Rejection: Session closed.
    Faulted,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Starting => "Starting",
            Self::Negotiating => "Negotiating",
            Self::Binding => "Binding",
            Self::Ready => "Ready",
            Self::Draining => "Draining",
            Self::Faulted => "Faulted",
        })
    }
}

/// The name of the rejection message.
pub fn message_name(frame: &provider::ProviderFrame) -> &'static str {
    use provider::provider_frame::Payload as P;
    match &frame.payload {
        Some(P::WorkerHello(_)) => "WorkerHello",
        Some(P::Initialize(_)) => "Initialize",
        Some(P::Initialized(_)) => "Initialized",
        Some(P::BindAuthenticator(_)) => "BindAuthenticator",
        Some(P::AuthenticatorReady(_)) => "AuthenticatorReady",
        Some(P::Describe(_)) => "Describe",
        Some(P::Described(_)) => "Described",
        Some(P::PrebindDescribe(_)) => "PrebindDescribe",
        Some(P::PrebindDescription(_)) => "PrebindDescription",
        Some(P::StartVerification(_)) => "StartVerification",
        Some(P::StartEnrollment(_)) => "StartEnrollment",
        Some(P::ContinueEnrollment(_)) => "ContinueEnrollment",
        Some(P::StartRemoval(_)) => "StartRemoval",
        Some(P::QueryMutation(_)) => "QueryMutation",
        Some(P::RequestCancel(_)) => "RequestCancel",
        Some(P::EnrollmentTransactionRequest(_)) => "EnrollmentTransactionRequest",
        Some(P::EnrollmentTransactionResult(_)) => "EnrollmentTransactionResult",
        Some(P::CommandAck(_)) => "CommandAck",
        Some(P::ProviderProgress(_)) => "ProviderProgress",
        Some(P::VerificationOutcome(_)) => "VerificationOutcome",
        Some(P::MutationOutcome(_)) => "MutationOutcome",
        Some(P::QueryMutationResult(_)) => "QueryMutationResult",
        Some(P::CancelReceipt(_)) => "CancelReceipt",
        Some(P::Quiesce(_)) => "Quiesce",
        Some(P::Quiesced(_)) => "Quiesced",
        Some(P::CloseAuthenticator(_)) => "CloseAuthenticator",
        Some(P::Shutdown(_)) => "Shutdown",
        Some(P::PrepareEnrollmentChallenge(_)) => "PrepareEnrollmentChallenge",
        Some(P::EnrollmentChallengeReady(_)) => "EnrollmentChallengeReady",
        Some(P::ProviderFault(_)) => "ProviderFault",
        None => "пусто",
    }
}

/// Whether the message is in this phase.
///
/// ⚠️ The list is set ** from the table ** of document 06 and does not think anything out.
/// `RequestCancel` It is accepted in any working phase: cancellation should not wait for the
/// queue.
pub fn accepts(phase: Phase, frame: &provider::ProviderFrame) -> bool {
    accepts_with_features(phase, frame, false)
}

/// A separate agreed extension of the declaration does not open access to hardware.
pub fn accepts_with_features(
    phase: Phase,
    frame: &provider::ProviderFrame,
    prebind_negotiated: bool,
) -> bool {
    use provider::provider_frame::Payload as P;
    if !(carosign_protocol::limits::MINOR_BASELINE..=carosign_protocol::limits::MINOR_MAX)
        .contains(&frame.minor)
    {
        return false;
    }
    match (&frame.payload, phase) {
        (_, Phase::Faulted) => false,
        (Some(P::Initialize(initialize)), Phase::Negotiating) => {
            initialize.selected_minor == frame.minor
                && frame.minor >= carosign_protocol::limits::MINOR_BASELINE
                && frame.minor <= carosign_protocol::limits::MINOR_MAX
        }
        (Some(P::BindAuthenticator(_)), Phase::Binding) => true,
        (Some(P::PrebindDescribe(_)), Phase::Binding) => prebind_negotiated,
        (Some(P::Describe(_)), Phase::Ready) => true,
        (Some(P::PrepareEnrollmentChallenge(_)), Phase::Ready) => {
            frame.minor >= carosign_protocol::limits::MINOR_BASELINE
                && frame.minor <= carosign_protocol::limits::MINOR_MAX
        }
        (
            Some(
                P::StartVerification(_)
                | P::StartEnrollment(_)
                | P::ContinueEnrollment(_)
                | P::StartRemoval(_)
                | P::QueryMutation(_),
            ),
            Phase::Ready,
        ) => {
            frame.minor >= carosign_protocol::limits::MINOR_BASELINE
                && frame.minor <= carosign_protocol::limits::MINOR_MAX
        }
        (
            Some(P::RequestCancel(_) | P::EnrollmentTransactionRequest(_)),
            Phase::Ready | Phase::Draining,
        ) => true,
        (Some(P::Quiesce(_)), Phase::Ready | Phase::Draining) => true,
        (Some(P::CloseAuthenticator(_) | P::Shutdown(_)), Phase::Draining) => true,
        _ => false,
    }
}

/// The next phase of the worker.
pub fn transition(phase: Phase, step: HandshakeStep) -> Phase {
    match (phase, step) {
        (Phase::Starting, HandshakeStep::SendHello) => Phase::Negotiating,
        (Phase::Negotiating, HandshakeStep::AwaitInitialize) => Phase::Negotiating,
        (Phase::Negotiating, HandshakeStep::SendInitialized) => Phase::Binding,
        (Phase::Binding, HandshakeStep::AwaitBind) => Phase::Binding,
        (Phase::Binding, HandshakeStep::SendReady) => Phase::Ready,
        (Phase::Ready, HandshakeStep::AcceptCommand) => Phase::Ready,
        _ => Phase::Faulted,
    }
}

#[path = "phase.tests.rs"]
#[cfg(test)]
mod tests;
