use super::{CheckedRequest, ReceiveRequest, SentReceipt};

use crate::relay_dispatch::PendingPacket;
use crate::relay_queue::{QueueError, RelayQueues};
use crate::transport::{PacketSocket, TransportError};
use carosign_protocol::carosign::provider::v1::ProviderFrame;
use prost::Message;

pub(crate) struct RelayPump {
    provider: PacketSocket,
    tunnel: PacketSocket,
    queues: RelayQueues,
    pending_command: Option<PendingPacket>,
    pending_event: Option<Vec<u8>>,
    command_count: usize,
    control_count: usize,
    command_bytes: usize,
    pending_command_bytes: usize,
    pending_is_control: bool,
    pending_request_id: Option<[u8; 16]>,
    sent_request_id: Option<SentReceipt>,
    sequenced: bool,
    event_count: usize,
    event_bytes: usize,
    closed: bool,
}

impl RelayPump {
    #[cfg(test)]
    pub(super) fn provider_for_test(&self) -> &PacketSocket {
        &self.provider
    }

    #[cfg(test)]
    pub(super) fn tunnel_for_test(&self) -> &PacketSocket {
        &self.tunnel
    }

    /// Cancellation confirmation cannot be issued while old events are held.
    pub(crate) fn has_pending_events(&self) -> bool {
        self.event_count != 0
    }
    pub(crate) fn new(provider: PacketSocket, tunnel: PacketSocket) -> Self {
        Self {
            provider,
            tunnel,
            queues: RelayQueues::default(),
            pending_command: None,
            pending_event: None,
            command_count: 0,
            control_count: 0,
            command_bytes: 0,
            pending_command_bytes: 0,
            pending_is_control: false,
            pending_request_id: None,
            sent_request_id: None,
            sequenced: false,
            event_count: 0,
            event_bytes: 0,
            closed: false,
        }
    }

    fn close(&mut self) {
        self.closed = true;
        self.pending_command = None;
        self.pending_request_id = None;
        self.pending_event = None;
        self.queues.close();
        self.provider.close();
        self.tunnel.close();
    }

    /// Accepts one package of tunnel only if there is a place in the queue.
    /// The verifier is required to check the original wire form and retrieve the provider
    /// frame; a successful verification is not a permission to send.
    pub(crate) fn receive_command(
        &mut self,
        validate: impl FnOnce(&[u8]) -> Result<Vec<u8>, ()>,
    ) -> Result<bool, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        // Do not read a packet if there is nowhere to keep it after reading it, considering
        // the maximum possible length and the packet already in the send.
        if self.command_count == 64
            || self.command_bytes + carosign_protocol::limits::FRAME_MAX > 8 * 1024 * 1024
        {
            return Ok(false);
        }
        let bytes = match self.tunnel.receive() {
            Ok(bytes) => bytes,
            Err(TransportError::WouldBlock) => return Err(TransportError::WouldBlock),
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        let checked = match validate(&bytes) {
            Ok(checked) => checked,
            Err(()) => {
                self.close();
                return Err(TransportError::Invalid);
            }
        };
        if self.command(checked).is_err() {
            self.close();
            return Err(TransportError::Invalid);
        }
        Ok(true)
    }

    /// Accepts a strictly verified calling tunnel without a frame number.
    /// The number is assigned only after the sender selects the queue, and the check of the
    /// original protobuf bytes and permissions remains the responsibility of the session
    /// owner.
    pub(crate) fn receive_sequenced_command(
        &mut self,
        worker: [u8; 16],
        epoch: u64,
        next_provider_seq: &mut u64,
        validate: impl FnOnce(&[u8]) -> Result<ProviderFrame, ()>,
    ) -> Result<bool, TransportError> {
        self.receive_command(|bytes| {
            let mut frame = validate(bytes)?;
            let payload = frame.payload.as_ref().ok_or(())?;
            if frame.frame_seq != 0
                || frame.worker_instance_id != worker
                || frame.device_epoch
                    != if matches!(
                        payload,
                        carosign_protocol::carosign::provider::v1::provider_frame::Payload::Initialize(
                            _
                        )
                    ) {
                        0
                    } else {
                        epoch
                    }
                || crate::relay_dispatch::direction(payload)
                    != crate::relay_dispatch::Direction::Command
                || *next_provider_seq == 0
            {
                return Err(());
            }
            frame.frame_seq = 1;
            carosign_protocol::validation::check_provider_envelope(&frame).map_err(|_| ())?;
            frame.frame_seq = 0;
            let bytes = frame.encode_to_vec();
            if bytes.len() > carosign_protocol::limits::FRAME_MAX {
                return Err(());
            }
            Ok(bytes)
        })
        .inspect(|queued| {
            if *queued {
                self.sequenced = true;
            }
        })
    }

    /// The owner checks the original wire and rights independently; this method keeps the
    /// command linked to the receipt until valid local delivery.
    pub(crate) fn receive_correlated_command(
        &mut self,
        worker: [u8; 16],
        epoch: u64,
        next_provider_seq: u64,
        validate: impl FnOnce(&[u8]) -> Result<([u8; 16], ProviderFrame), ()>,
    ) -> Result<ReceiveRequest, TransportError> {
        self.receive_request(|raw| {
            let (request_id, mut frame) = validate(raw)?;
            let payload = frame.payload.as_ref().ok_or(())?;
            if frame.frame_seq != 0
                || frame.worker_instance_id != worker
                || frame.device_epoch
                    != if matches!(
                        payload,
                        carosign_protocol::carosign::provider::v1::provider_frame::Payload::Initialize(
                            _
                        )
                    ) {
                        0
                    } else {
                        epoch
                    }
                || crate::relay_dispatch::direction(payload)
                    != crate::relay_dispatch::Direction::Command
                || next_provider_seq == 0
            {
                return Err(());
            }
            frame.frame_seq = 1;
            carosign_protocol::validation::check_provider_envelope(&frame).map_err(|_| ())?;
            frame.frame_seq = 0;
            Ok(CheckedRequest::Command {
                request_id,
                bytes: frame.encode_to_vec(),
            })
        })
        .inspect(|queued| {
            if matches!(queued, ReceiveRequest::Queued { .. }) {
                self.sequenced = true;
            }
        })
    }

    /// The control has already been verified by the owner; the frame number has not yet been
    /// assigned.
    pub(crate) fn sequenced_control(&mut self, frame: ProviderFrame) -> Result<(), QueueError> {
        if frame.frame_seq != 0
            || frame.payload.as_ref().is_none_or(|p| {
                crate::relay_dispatch::direction(p)
                    != crate::relay_dispatch::Direction::BrokerCancel
            })
        {
            self.close();
            return Err(QueueError::InvalidFrame);
        }
        self.control(frame.encode_to_vec())?;
        self.sequenced = true;
        Ok(())
    }

    /// The Owner verifies the origin and authority of all FDs before transfer.
    /// Descriptors go in line and close when a recall or error occurs.
    pub(crate) fn sequenced_bind(
        &mut self,
        frame: ProviderFrame,
        fds: Vec<std::os::fd::OwnedFd>,
    ) -> Result<(), QueueError> {
        use carosign_protocol::carosign::provider::v1::{FdRole, provider_frame::Payload};
        let valid = match &frame.payload {
            Some(Payload::BindAuthenticator(bind)) => {
                frame.frame_seq == 0
                    && fds.len() <= 8
                    && bind.fd_slots.len() == fds.len()
                    && bind.fd_slots.iter().enumerate().all(|(i, slot)| {
                        slot.index as usize == i
                            && slot.resource_id.len() == 16
                            && slot.resource_id.iter().any(|b| *b != 0)
                            && !bind.fd_slots[..i]
                                .iter()
                                .any(|previous| previous.resource_id == slot.resource_id)
                            && matches!(
                                FdRole::from_i32(slot.role),
                                Some(
                                    FdRole::Device
                                        | FdRole::Event
                                        | FdRole::Config
                                        | FdRole::MediatedService
                                )
                            )
                    })
            }
            _ => false,
        };
        if !valid {
            self.close();
            return Err(QueueError::InvalidFrame);
        }
        if self.closed {
            return Err(QueueError::Closed);
        }
        let bytes = frame.encode_to_vec();
        if self.command_count == 64 || self.command_bytes + bytes.len() > 8 * 1024 * 1024 {
            return Err(QueueError::Busy);
        }
        let len = bytes.len();
        if let Err(error) = self.queues.command_with_fds(bytes, fds, true) {
            // The error of the queue limit is the permissible refusal before sending, the
            // incorrect frame violates the tunnel contract and closes the entire session.
            if error != QueueError::Busy {
                self.close();
            }
            return Err(error);
        }
        self.command_count += 1;
        self.command_bytes += len;
        self.sequenced = true;
        Ok(())
    }

    /// Accepts one employee event through a verifier wire owner.
    /// If it is impossible to hold the event, it closes both channels: the event could be
    /// terminal and should not be lost unnoticed.
    pub(crate) fn receive_event(
        &mut self,
        validate: impl FnOnce(&[u8]) -> Result<Vec<u8>, ()>,
    ) -> Result<bool, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        let bytes = match self.provider.receive() {
            Ok(bytes) => bytes,
            Err(TransportError::WouldBlock) => return Err(TransportError::WouldBlock),
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        let checked = match validate(&bytes) {
            Ok(checked) => checked,
            Err(()) => {
                self.close();
                return Err(TransportError::Invalid);
            }
        };
        if self.event(checked).is_err() {
            self.close();
            return Err(TransportError::Invalid);
        }
        Ok(true)
    }

    /// Accepts and classifies one packet of general tunnel. The examiner is obliged to
    /// disassemble the original wire and check the area and authority; reception does not
    /// confirm the recall or sending. Overflow of control closes the session.
    pub(crate) fn receive_request(
        &mut self,
        validate: impl FnOnce(&[u8]) -> Result<CheckedRequest, ()>,
    ) -> Result<ReceiveRequest, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        let bytes = match self.tunnel.receive() {
            Ok(bytes) => bytes,
            Err(TransportError::WouldBlock) => return Err(TransportError::WouldBlock),
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        let checked = match validate(&bytes) {
            Ok(checked) => checked,
            Err(()) => {
                self.close();
                return Err(TransportError::Invalid);
            }
        };
        if let CheckedRequest::Local { request_id } = checked {
            if request_id.iter().all(|byte| *byte == 0) {
                self.close();
                return Err(TransportError::Invalid);
            }
            return Ok(ReceiveRequest::Local { request_id });
        }
        let (request_id, bytes, control, fds, bind) = match checked {
            CheckedRequest::Local { .. } => unreachable!(),
            CheckedRequest::Command { request_id, bytes } => {
                (request_id, bytes, false, Vec::new(), false)
            }
            CheckedRequest::Control { request_id, bytes } => {
                (request_id, bytes, true, Vec::new(), false)
            }
            CheckedRequest::Bind {
                request_id,
                bytes,
                fds,
            } => (request_id, bytes, false, fds, true),
        };
        if request_id.iter().all(|byte| *byte == 0) {
            self.close();
            return Err(TransportError::Invalid);
        }
        let len = bytes.len();
        let queued = if control {
            if self.control_count == 8 || self.command_bytes + len > 8 * 1024 * 1024 {
                self.close();
                return Err(TransportError::Invalid);
            }
            self.queues.control_correlated(bytes, Some(request_id))
        } else {
            if self.command_count == 64 || self.command_bytes + len > 8 * 1024 * 1024 {
                return Ok(ReceiveRequest::Busy { request_id });
            }
            self.queues
                .command_correlated(bytes, fds, bind, Some(request_id))
        };
        match queued {
            Ok(()) => {
                if control {
                    self.control_count += 1;
                } else {
                    self.command_count += 1;
                }
                self.command_bytes += len;
                Ok(ReceiveRequest::Queued { request_id })
            }
            Err(QueueError::Busy) => Ok(ReceiveRequest::Busy { request_id }),
            Err(_) => {
                self.close();
                Err(TransportError::Invalid)
            }
        }
    }

    /// The package has already passed wire and direction testing. It's not right to send.
    pub(crate) fn command(&mut self, bytes: Vec<u8>) -> Result<(), QueueError> {
        if self.closed {
            return Err(QueueError::Closed);
        }
        if self.command_count == 64 || self.command_bytes + bytes.len() > 8 * 1024 * 1024 {
            return Err(QueueError::Busy);
        }
        let len = bytes.len();
        self.queues.command(bytes)?;
        self.command_count += 1;
        self.command_bytes += len;
        Ok(())
    }

    /// The control is already vetted by the session owner, and overflow closes the
    /// connections, so you can't lose the recall or cancel.
    pub(crate) fn control(&mut self, bytes: Vec<u8>) -> Result<(), QueueError> {
        if self.closed {
            return Err(QueueError::Closed);
        }
        if self.control_count == 8 || self.command_bytes + bytes.len() > 8 * 1024 * 1024 {
            self.close();
            return Err(QueueError::Closed);
        }
        let len = bytes.len();
        if let Err(error) = self.queues.control(bytes) {
            self.close();
            return Err(error);
        }
        self.control_count += 1;
        self.command_bytes += len;
        Ok(())
    }

    /// The event has already passed the direction and sequence test.
    pub(crate) fn event(&mut self, bytes: Vec<u8>) -> Result<(), QueueError> {
        if self.closed {
            return Err(QueueError::Closed);
        }
        if self.event_count == 64 || self.event_bytes + bytes.len() > 8 * 1024 * 1024 {
            self.close();
            return Err(QueueError::Closed);
        }
        let len = bytes.len();
        let result = self.queues.event(bytes);
        if result.is_err() {
            self.close();
        } else {
            self.event_count += 1;
            self.event_bytes += len;
        }
        result
    }

    /// The recall cancels the unsubmitted shipment and closes both channels.
    /// The caller is responsible for fencing all affected areas.
    pub(crate) fn revoke(&mut self) {
        if let Some(pending) = &mut self.pending_command {
            let _ = pending.revoke();
        }
        self.close();
    }

    /// Non-blocking attempt; the validation must be repeated on each.
    pub(crate) fn send_command(&mut self, authorized: bool) -> Result<bool, TransportError> {
        if self.sequenced {
            self.close();
            return Err(TransportError::Invalid);
        }
        self.send_queued(authorized, None)
    }

    /// Selects a priority queue before a number is assigned, and after EAGAIN, the same
    /// number remains with the withheld packet until it is sent or recalled.
    pub(crate) fn send_sequenced_command(
        &mut self,
        authorized: bool,
        next: &mut u64,
    ) -> Result<bool, TransportError> {
        self.send_queued(authorized, Some(next))
    }

    fn send_queued(
        &mut self,
        authorized: bool,
        mut next: Option<&mut u64>,
    ) -> Result<bool, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        // The receipt refers to a specific shipment and cannot be overwritten by the next one
        // until the owner has taken it away.
        if self.sent_request_id.is_some() {
            return Err(TransportError::WouldBlock);
        }
        if self.pending_command.is_none() {
            let Some((is_control, packet)) = self.queues.next_tagged_request() else {
                return Ok(false);
            };
            let mut bytes = packet.bytes;
            let original_len = bytes.len();
            if let Some(sequence) = next.as_mut() {
                let mut frame = match ProviderFrame::decode(bytes.as_slice()) {
                    Ok(frame) if frame.frame_seq == 0 && **sequence != 0 => frame,
                    _ => {
                        self.close();
                        return Err(TransportError::Invalid);
                    }
                };
                let Some(following) = sequence.checked_add(1) else {
                    self.close();
                    return Err(TransportError::Invalid);
                };
                frame.frame_seq = **sequence;
                if carosign_protocol::validation::check_provider_envelope(&frame).is_err() {
                    self.close();
                    return Err(TransportError::Invalid);
                }
                bytes = frame.encode_to_vec();
                if bytes.len() > carosign_protocol::limits::FRAME_MAX {
                    self.close();
                    return Err(TransportError::Invalid);
                }
                **sequence = following;
            } else if self.sequenced {
                self.close();
                return Err(TransportError::Invalid);
            }
            self.pending_is_control = is_control;
            self.pending_request_id = packet.request_id;
            self.pending_command_bytes = original_len;
            self.pending_command = Some(if packet.is_bind {
                PendingPacket::bind(bytes, packet.fds)?
            } else {
                PendingPacket::new(bytes)?
            });
        }
        let result = self
            .pending_command
            .as_mut()
            .expect("пакет удерживается")
            .attempt(&self.provider, authorized);
        match result {
            Ok(()) => {
                self.pending_command = None;
                self.sent_request_id =
                    self.pending_request_id
                        .take()
                        .map(|request_id| SentReceipt {
                            request_id,
                            is_control: self.pending_is_control,
                        });
                if self.pending_is_control {
                    self.control_count -= 1;
                } else {
                    self.command_count -= 1;
                }
                self.pending_is_control = false;
                self.command_bytes -= self.pending_command_bytes;
                self.pending_command_bytes = 0;
                Ok(true)
            }
            Err(TransportError::WouldBlock) => Err(TransportError::WouldBlock),
            Err(error) => {
                self.close();
                Err(error)
            }
        }
    }

    /// Returns the ID of the only confirmed local shipment.
    /// This is not an acknowledgement of worker processing or device rest.
    pub(crate) fn take_sent_request_id(&mut self) -> Option<[u8; 16]> {
        self.take_sent_receipt().map(|receipt| receipt.request_id)
    }

    /// Returns a typed receipt so that the control does not turn into a confirmation of a
    /// normal command when forming a tunnel response.
    pub(crate) fn take_sent_receipt(&mut self) -> Option<SentReceipt> {
        self.sent_request_id.take()
    }

    /// The number belongs to the owner of the stream and is promoted only after successful
    /// staging; if you can not hold the response, the session closes.
    /// This receipt does not confirm the processing of the command or rest of the device.
    pub(crate) fn queue_sent_result(
        &mut self,
        worker: [u8; 16],
        epoch: u64,
        next_tunnel_seq: &mut u64,
    ) -> Result<bool, QueueError> {
        use carosign_protocol::carosign::broker_relay::v1::{
            CommandResult, Frame, command_result, frame::Body,
        };

        if self.closed {
            return Err(QueueError::Closed);
        }
        let Some(receipt) = self.sent_request_id else {
            return Ok(false);
        };
        let Some(following) = next_tunnel_seq.checked_add(1) else {
            self.close();
            return Err(QueueError::InvalidFrame);
        };
        if worker.iter().all(|byte| *byte == 0) || epoch == 0 || *next_tunnel_seq == 0 {
            self.close();
            return Err(QueueError::InvalidFrame);
        }
        // Control can mean Cancel or Revoke, and unless the owner delivers a proven control,
        // you can't choose the answer.
        if receipt.is_control {
            self.close();
            return Err(QueueError::InvalidFrame);
        }
        let body = Body::CommandResult(CommandResult {
            request_id: receipt.request_id.to_vec(),
            result: Some(command_result::Result::Sent(true)),
        });
        let frame = Frame {
            major: 1,
            minor: 0,
            seq: *next_tunnel_seq,
            worker_instance: worker.to_vec(),
            device_epoch: epoch,
            body: Some(body),
        };
        self.event(frame.encode_to_vec())?;
        self.sent_request_id = None;
        *next_tunnel_seq = following;
        Ok(true)
    }

    /// A terminal event is not lost by EAGAIN and is not replaced by a new one.
    pub(crate) fn send_event(&mut self) -> Result<bool, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        if self.pending_event.is_none() {
            self.pending_event = self.queues.next_event();
        }
        let Some(event) = &self.pending_event else {
            return Ok(false);
        };
        match self.tunnel.send(event) {
            Ok(()) => {
                self.event_count -= 1;
                self.event_bytes -= event.len();
                self.pending_event = None;
                Ok(true)
            }
            Err(TransportError::WouldBlock) => Err(TransportError::WouldBlock),
            Err(error) => {
                self.close();
                Err(error)
            }
        }
    }
}
