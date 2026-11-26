//! Verification of the ownership and order of events of one changing operation.

use super::transport::{Transport, TransportError};
use carosign_protocol::carosign::client::v1::{
    ClientFrame, EventKind, MutationResult, MutationTerminal, OperationAccepted, OperationEvent,
    client_frame::Payload, terminal_result::Kind,
};

/// Surveillance without claims of trust in the connection source.
pub fn observe_mutation_with_acceptance(
    transport: &Transport,
    request: Payload,
    accepted_operation: impl FnOnce(&[u8]),
) -> Result<MutationTerminal, TransportError> {
    observe_mutation_with_events(transport, request, accepted_operation, |_| {})
}

pub fn observe_mutation_with_events(
    transport: &Transport,
    request: Payload,
    accepted_operation: impl FnOnce(&[u8]),
    mut progress: impl FnMut(&OperationEvent),
) -> Result<MutationTerminal, TransportError> {
    if !matches!(
        &request,
        Payload::BeginEnrollment(_) | Payload::BeginRemoval(_)
    ) {
        return Err(TransportError::Unexpected);
    }
    let response = transport.request(request)?;
    let Some(Payload::OperationAccepted(accepted)) = response.payload else {
        return Err(TransportError::Unavailable);
    };
    let mut tracker = MutationEvents::new(&accepted).map_err(|_| TransportError::Unexpected)?;
    accepted_operation(&accepted.operation_id);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    for _ in 0..4096 {
        let frame = transport.receive_operation_event_until(&tracker.operation_id, deadline)?;
        match tracker.accept(&frame) {
            Ok(Some(terminal)) => return Ok(terminal),
            Ok(None) => {
                if let Some(Payload::OperationEvent(event)) = &frame.payload {
                    progress(event);
                }
            }
            Err(()) => {
                transport.close();
                return Err(TransportError::Unexpected);
            }
        }
    }
    transport.close();
    Err(TransportError::Unavailable)
}

/// Area and order of the confirmed operation.
#[allow(clippy::result_unit_err)]
pub struct MutationEvents {
    operation_id: Vec<u8>,
    next: u64,
    terminal: bool,
}

#[allow(clippy::result_unit_err)]
impl MutationEvents {
    pub fn new(accepted: &OperationAccepted) -> Result<Self, ()> {
        if accepted.operation_id.len() != 16
            || accepted.operation_id.iter().all(|b| *b == 0)
            || accepted.first_event_sequence == 0
        {
            return Err(());
        }
        Ok(Self {
            operation_id: accepted.operation_id.clone(),
            next: accepted.first_event_sequence,
            terminal: false,
        })
    }

    pub fn accept(&mut self, frame: &ClientFrame) -> Result<Option<MutationTerminal>, ()> {
        let result = self.accept_inner(frame);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }

    fn accept_inner(&mut self, frame: &ClientFrame) -> Result<Option<MutationTerminal>, ()> {
        let Some(Payload::OperationEvent(event)) = &frame.payload else {
            return Err(());
        };
        if self.terminal
            || frame.operation_id != self.operation_id
            || event.operation_id != self.operation_id
            || frame.event_sequence != self.next
            || event.sequence != self.next
        {
            return Err(());
        }
        let terminal = event.kind == EventKind::EventTerminal as i32;
        if !terminal {
            if event.terminal.is_some()
                || ![
                    EventKind::EventStateChanged as i32,
                    EventKind::EventPlaceFinger as i32,
                    EventKind::EventMoveFinger as i32,
                    EventKind::EventRemoveFinger as i32,
                    EventKind::EventRetry as i32,
                    EventKind::EventWorking as i32,
                    EventKind::EventProgress as i32,
                ]
                .contains(&event.kind)
            {
                return Err(());
            }
            self.next = self.next.checked_add(1).ok_or(())?;
            return Ok(None);
        }
        let Some(Kind::Mutation(result)) = event.terminal.as_ref().and_then(|v| v.kind.as_ref())
        else {
            return Err(());
        };
        match MutationResult::from_i32(result.result) {
            Some(MutationResult::MutationCommitted) => {
                let enrollment = result.enrollment.as_ref().ok_or(())?;
                if enrollment.enrollment_id.len() != 16
                    || enrollment.enrollment_id.iter().all(|b| *b == 0)
                    || enrollment.authenticator_id.len() != 16
                    || enrollment.authenticator_id.iter().all(|b| *b == 0)
                    || enrollment.generation == 0
                {
                    return Err(());
                }
            }
            Some(MutationResult::MutationNeedsReconciliation) => {
                if result.mutation_id.len() != 16 || result.mutation_id.iter().all(|b| *b == 0) {
                    return Err(());
                }
            }
            Some(MutationResult::MutationNoChange | MutationResult::MutationFailed) => {}
            _ => return Err(()),
        }
        self.terminal = true;
        Ok(Some(result.clone()))
    }
}

#[path = "mutation_events.tests.rs"]
#[cfg(test)]
mod tests;
