use super::{OBSERVER_QUEUE_MAX, ObserverQueueError};

/// A limited, non-blocking sequence of outgoing events by one observer.
///
/// When overflowing, the queue closes; the magazine provides stability to the terminal
/// outcome, not this queue; the owner of the connection is obliged to close it and re-check
/// the authority upon new observation.
#[derive(Debug)]
pub struct ObserverQueue {
    events: std::collections::VecDeque<super::super::events::Event>,
    last_sequence: u64,
    disconnected: bool,
}

impl ObserverQueue {
    /// Create a queue after the number already delivered.
    pub fn new(after_sequence: u64) -> Self {
        Self {
            events: std::collections::VecDeque::new(),
            last_sequence: after_sequence,
            disconnected: false,
        }
    }

    /// Stages an event without waiting for the client; when filling requires a break.
    pub fn push(&mut self, event: super::super::events::Event) -> Result<(), ObserverQueueError> {
        if self.disconnected {
            return Err(ObserverQueueError::Disconnect);
        }
        if self.last_sequence.checked_add(1) != Some(event.sequence) {
            return Err(ObserverQueueError::OutOfOrder);
        }
        if self.events.len() == OBSERVER_QUEUE_MAX {
            self.disconnected = true;
            self.events.clear();
            return Err(ObserverQueueError::Disconnect);
        }
        self.last_sequence = event.sequence;
        self.events.push_back(event);
        Ok(())
    }

    /// Takes the event to send if the connection is still open.
    pub fn pop(&mut self) -> Option<super::super::events::Event> {
        if self.disconnected {
            None
        } else {
            self.events.pop_front()
        }
    }

    pub(crate) fn peek(&self) -> Option<&super::super::events::Event> {
        if self.disconnected {
            None
        } else {
            self.events.front()
        }
    }
}

#[cfg(test)]
#[path = "observer_queue.tests.rs"]
mod tests;
