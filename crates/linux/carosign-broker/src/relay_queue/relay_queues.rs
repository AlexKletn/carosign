use super::{QueueError, QueuedPacket};

use std::collections::VecDeque;
use std::os::fd::OwnedFd;

const MAX_FRAME: usize = 65_536;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMMANDS: usize = 64;
const MAX_CONTROLS: usize = 8;
const MAX_EVENTS: usize = 64;

#[derive(Default)]
pub(crate) struct RelayQueues {
    commands: VecDeque<QueuedPacket>,
    controls: VecDeque<QueuedPacket>,
    events: VecDeque<Vec<u8>>,
    incoming_bytes: usize,
    outgoing_bytes: usize,
    closed: bool,
}

impl RelayQueues {
    fn valid(frame: &[u8]) -> bool {
        !frame.is_empty() && frame.len() <= MAX_FRAME
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.commands.clear();
        self.controls.clear();
        self.events.clear();
        self.incoming_bytes = 0;
        self.outgoing_bytes = 0;
    }

    pub(crate) fn command(&mut self, frame: Vec<u8>) -> Result<(), QueueError> {
        self.command_with_fds(frame, Vec::new(), false)
    }

    pub(crate) fn command_with_fds(
        &mut self,
        frame: Vec<u8>,
        fds: Vec<OwnedFd>,
        is_bind: bool,
    ) -> Result<(), QueueError> {
        self.command_correlated(frame, fds, is_bind, None)
    }

    pub(crate) fn command_correlated(
        &mut self,
        frame: Vec<u8>,
        fds: Vec<OwnedFd>,
        is_bind: bool,
        request_id: Option<[u8; 16]>,
    ) -> Result<(), QueueError> {
        if self.closed {
            return Err(QueueError::Closed);
        }
        if !Self::valid(&frame) {
            return Err(QueueError::InvalidFrame);
        }
        if self.commands.len() == MAX_COMMANDS || self.incoming_bytes + frame.len() > MAX_BYTES {
            return Err(QueueError::Busy);
        }
        self.incoming_bytes += frame.len();
        self.commands.push_back(QueuedPacket {
            bytes: frame,
            fds,
            is_bind,
            request_id,
        });
        Ok(())
    }

    pub(crate) fn control(&mut self, frame: Vec<u8>) -> Result<(), QueueError> {
        self.control_correlated(frame, None)
    }

    pub(crate) fn control_correlated(
        &mut self,
        frame: Vec<u8>,
        request_id: Option<[u8; 16]>,
    ) -> Result<(), QueueError> {
        if self.closed {
            return Err(QueueError::Closed);
        }
        if !Self::valid(&frame) {
            self.close();
            return Err(QueueError::InvalidFrame);
        }
        if self.controls.len() == MAX_CONTROLS || self.incoming_bytes + frame.len() > MAX_BYTES {
            self.close();
            return Err(QueueError::Closed);
        }
        self.incoming_bytes += frame.len();
        self.controls.push_back(QueuedPacket {
            bytes: frame,
            fds: Vec::new(),
            is_bind: false,
            request_id,
        });
        Ok(())
    }

    pub(crate) fn event(&mut self, frame: Vec<u8>) -> Result<(), QueueError> {
        if self.closed {
            return Err(QueueError::Closed);
        }
        if !Self::valid(&frame) {
            self.close();
            return Err(QueueError::InvalidFrame);
        }
        if self.events.len() == MAX_EVENTS || self.outgoing_bytes + frame.len() > MAX_BYTES {
            self.close();
            return Err(QueueError::Closed);
        }
        self.outgoing_bytes += frame.len();
        self.events.push_back(frame);
        Ok(())
    }

    /// The management priority is valid until the next provider-send start.
    pub(crate) fn next_request(&mut self) -> Option<Vec<u8>> {
        self.next_tagged_request().map(|(_, frame)| frame.bytes)
    }

    pub(crate) fn next_tagged_request(&mut self) -> Option<(bool, QueuedPacket)> {
        let (control, frame) = match self.controls.pop_front() {
            Some(frame) => (true, frame),
            None => (false, self.commands.pop_front()?),
        };
        self.incoming_bytes -= frame.bytes.len();
        Some((control, frame))
    }

    pub(crate) fn next_event(&mut self) -> Option<Vec<u8>> {
        let frame = self.events.pop_front()?;
        self.outgoing_bytes -= frame.len();
        Some(frame)
    }
}

#[path = "relay_queues.tests.rs"]
#[cfg(test)]
mod tests;
