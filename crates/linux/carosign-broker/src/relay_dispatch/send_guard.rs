use super::{Direction, InvalidTransition, SendState};

use carosign_protocol::carosign::provider::v1::provider_frame::Payload;

/// Classifies the closed list of provider v1.3.
pub fn direction(payload: &Payload) -> Direction {
    match payload {
        Payload::Initialize(_)
        | Payload::Describe(_)
        | Payload::PrebindDescribe(_)
        | Payload::StartVerification(_)
        | Payload::PrepareEnrollmentChallenge(_)
        | Payload::StartEnrollment(_)
        | Payload::ContinueEnrollment(_)
        | Payload::StartRemoval(_)
        | Payload::QueryMutation(_)
        | Payload::Quiesce(_)
        | Payload::CloseAuthenticator(_)
        | Payload::Shutdown(_) => Direction::Command,
        Payload::BindAuthenticator(_) => Direction::Bind,
        Payload::RequestCancel(_) => Direction::BrokerCancel,
        Payload::EnrollmentTransactionRequest(_) => Direction::EnrollmentTransaction,
        Payload::WorkerHello(_)
        | Payload::Initialized(_)
        | Payload::AuthenticatorReady(_)
        | Payload::Described(_)
        | Payload::PrebindDescription(_)
        | Payload::CommandAck(_)
        | Payload::ProviderProgress(_)
        | Payload::VerificationOutcome(_)
        | Payload::MutationOutcome(_)
        | Payload::QueryMutationResult(_)
        | Payload::CancelReceipt(_)
        | Payload::EnrollmentTransactionResult(_)
        | Payload::EnrollmentChallengeReady(_)
        | Payload::Quiesced(_)
        | Payload::ProviderFault(_) => Direction::Event,
    }
}

/// Serialized sending stage. Repeat after recording is prohibited.
#[derive(Debug)]
pub struct SendGuard {
    state: SendState,
}

impl Default for SendGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SendGuard {
    pub fn new() -> Self {
        Self {
            state: SendState::Prepared,
        }
    }

    pub fn state(&self) -> SendState {
        self.state
    }

    /// The transition should be performed under the same serialization as the review.
    pub fn begin(&mut self) -> Result<(), InvalidTransition> {
        if self.state != SendState::Prepared {
            return Err(InvalidTransition);
        }
        self.state = SendState::Sending;
        Ok(())
    }

    /// Confirmed exactly one full local shipment.
    pub fn sent(&mut self) -> Result<(), InvalidTransition> {
        if self.state != SendState::Sending {
            return Err(InvalidTransition);
        }
        self.state = SendState::Sent;
        Ok(())
    }

    /// The kernel returned EAGAIN to the record; the same pending shipment is retained.
    /// It can be re-transmitted only after a new review and deadline.
    pub fn would_block(&mut self) -> Result<(), InvalidTransition> {
        if self.state != SendState::Sending {
            return Err(InvalidTransition);
        }
        self.state = SendState::Prepared;
        Ok(())
    }

    /// The pre-recorded revocation prohibits it; after the beginning, the outcome is unclear.
    pub fn revoke(&mut self) -> Result<(), InvalidTransition> {
        self.state = match self.state {
            SendState::Prepared => SendState::Revoked,
            SendState::Sending | SendState::Sent => SendState::Uncertain,
            SendState::Revoked | SendState::Uncertain => return Err(InvalidTransition),
        };
        Ok(())
    }

    /// Loss of communication after a possible record does not turn into a failure before
    /// sending.
    pub fn lost(&mut self) -> Result<(), InvalidTransition> {
        if !matches!(self.state, SendState::Sending | SendState::Sent) {
            return Err(InvalidTransition);
        }
        self.state = SendState::Uncertain;
        Ok(())
    }
}

#[path = "send_guard.tests.rs"]
#[cfg(test)]
mod tests;
