use super::InvalidEvent;

use carosign_protocol::carosign::client::v1::{
    ClientFrame, EventKind, OperationAccepted, VerificationResult, client_frame::Payload,
    terminal_result::Kind,
};

/// Status of one accepted operation on unconfirmed transport.
pub struct VerificationEvents {
    operation_id: Vec<u8>,
    next: u64,
    terminal: bool,
}

impl VerificationEvents {
    pub(crate) fn operation_id(&self) -> &[u8] {
        &self.operation_id
    }

    /// Creates an auditor from the confirmation of the operation, without issuing authority.
    pub fn new(accepted: &OperationAccepted) -> Result<Self, InvalidEvent> {
        if accepted.operation_id.len() != 16
            || accepted.operation_id.iter().all(|byte| *byte == 0)
            || accepted.first_event_sequence == 0
        {
            return Err(InvalidEvent);
        }
        Ok(Self {
            operation_id: accepted.operation_id.clone(),
            next: accepted.first_event_sequence,
            terminal: false,
        })
    }

    /// Checks the event and returns the fact of the terminal without permission to enter.
    /// The pass requires separate synchronization and is not accepted here.
    pub fn accept(&mut self, frame: &ClientFrame) -> Result<bool, InvalidEvent> {
        let result = self.accept_inner(frame);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }

    fn accept_inner(&mut self, frame: &ClientFrame) -> Result<bool, InvalidEvent> {
        let Some(Payload::OperationEvent(event)) = &frame.payload else {
            return Err(InvalidEvent);
        };
        if self.terminal
            || frame.operation_id != self.operation_id
            || event.operation_id != self.operation_id
            || frame.event_sequence != self.next
            || event.sequence != self.next
        {
            return Err(InvalidEvent);
        }
        if ![
            EventKind::EventStateChanged as i32,
            EventKind::EventPlaceFinger as i32,
            EventKind::EventMoveFinger as i32,
            EventKind::EventRemoveFinger as i32,
            EventKind::EventRetry as i32,
            EventKind::EventWorking as i32,
            EventKind::EventProgress as i32,
            EventKind::EventTerminal as i32,
        ]
        .contains(&event.kind)
        {
            return Err(InvalidEvent);
        }
        let terminal = event.kind == EventKind::EventTerminal as i32;
        if terminal {
            let Some(Kind::Verification(result)) =
                event.terminal.as_ref().and_then(|t| t.kind.as_ref())
            else {
                return Err(InvalidEvent);
            };
            if ![
                VerificationResult::VerifyFactorAccepted as i32,
                VerificationResult::VerifyNotMatched as i32,
                VerificationResult::VerifyNoSample as i32,
                VerificationResult::VerifyCancelled as i32,
                VerificationResult::VerifyUnavailable as i32,
                VerificationResult::VerifyFailed as i32,
            ]
            .contains(&result.result)
            {
                return Err(InvalidEvent);
            }
        } else if event.terminal.is_some() {
            return Err(InvalidEvent);
        }
        // After the terminal, there will be no next number: the last allowable flow number
        // may also belong to the terminal event.
        if !terminal {
            self.next = self.next.checked_add(1).ok_or(InvalidEvent)?;
        }
        self.terminal = terminal;
        Ok(terminal)
    }
}

#[path = "sequence_tracker.tests.rs"]
#[cfg(test)]
mod tests;
