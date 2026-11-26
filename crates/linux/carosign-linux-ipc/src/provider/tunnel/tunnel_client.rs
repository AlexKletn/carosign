use super::{TunnelCommandEncoder, TunnelError, TunnelHello};

use super::event_validation::strict_event;
use super::result_validation::{strict_command_result, strict_result, strict_sent_result};

use std::collections::VecDeque;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

use carosign_protocol::carosign::broker_relay::v1::{
    Frame, MutationPermit, Scope, Slot, bind_result, command_result, control_result, frame::Body,
};
use carosign_protocol::carosign::common::v1::CapabilityDescriptor;
use carosign_protocol::carosign::provider::v1::{Feature, ProviderFrame, provider_frame::Payload};
use prost::Message;
use zeroize::Zeroizing;

/// Owner of a private channel for initial exchange and regular commands.
///
/// The Hello match doesn't certify the middleman. Only explicitly verified events are
/// supported; Bind and Control aren't connected yet.
pub struct TunnelClient {
    fd: OwnedFd,
    encoder: TunnelCommandEncoder,
    worker: [u8; 16],
    epoch: u64,
    next_in: u64,
    provider_in: carosign_provider_sdk::wire::Channel,
    pending_events: VecDeque<ProviderFrame>,
    pending_authenticator: Option<carosign_protocol::carosign::common::v1::AuthenticatorRef>,
    pending_prebind: Option<(
        Vec<u8>,
        carosign_protocol::carosign::common::v1::AuthenticatorRef,
    )>,
    pending_challenge: Option<(Vec<u8>, Vec<u8>, Vec<u8>)>,
    capture_feedback_enabled: bool,
    capture_device: Option<CapabilityDescriptor>,
    closed: bool,
}

impl TunnelClient {
    /// Closes the channel after the owner of the transaction refuses; re-exchange is
    /// prohibited.
    pub fn abort(&mut self) {
        self.close();
    }

    /// Accepts a private socket issued by the supervisor; confirmation of its origin and life
    /// cycle remains with the system owner.
    pub fn connect(
        fd: OwnedFd,
        worker: [u8; 16],
        epoch: u64,
        broker: [u8; 16],
        boot: [u8; 16],
        timeout: Duration,
    ) -> Result<Self, TunnelError> {
        let mut kind: libc::c_int = 0;
        let mut len = std::mem::size_of_val(&kind) as libc::socklen_t;
        // SAFETY: The buffer is valid during the call.
        if unsafe {
            libc::getsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut libc::c_int).cast(),
                &mut len,
            )
        } != 0
            || kind != libc::SOCK_SEQPACKET
            || !super::super::transport::unnamed_unix_address(fd.as_raw_fd(), false)
            || !super::super::transport::unnamed_unix_address(fd.as_raw_fd(), true)
        {
            return Err(TunnelError::Invalid);
        }
        // SAFETY: The descriptor belongs to the calling object.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
        {
            return Err(TunnelError::Io(std::io::Error::last_os_error()));
        }
        let mut hello =
            TunnelHello::new(worker, epoch, broker, boot).map_err(|_| TunnelError::Invalid)?;
        let mut client = Self {
            fd,
            encoder: TunnelCommandEncoder::after_hello(worker, epoch, broker)
                .map_err(|_| TunnelError::Invalid)?,
            worker,
            epoch,
            next_in: 2,
            provider_in: carosign_provider_sdk::wire::Channel::new(),
            pending_events: VecDeque::new(),
            pending_authenticator: None,
            pending_prebind: None,
            pending_challenge: None,
            capture_feedback_enabled: false,
            capture_device: None,
            closed: false,
        };
        let result = (|| {
            client.send_packet(
                &hello.outgoing().map_err(|_| TunnelError::Invalid)?,
                timeout,
            )?;
            let reply = client.receive_packet(timeout)?;
            hello.accept(&reply, 0).map_err(|_| TunnelError::Invalid)
        })();
        if result.is_err() {
            client.close();
        }
        result.map(|()| client)
    }

    /// Accepts the next worker event: An unsupported event, a breach of context, or an
    /// ambiguous wire closes the channel.
    pub fn receive_event(&mut self, timeout: Duration) -> Result<ProviderFrame, TunnelError> {
        if self.closed {
            return Err(TunnelError::Closed);
        }
        if let Some(frame) = self.pending_events.pop_front() {
            return Ok(frame);
        }
        let result = (|| {
            let bytes = self.receive_packet(timeout)?;
            self.parse_event(&bytes)
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    fn parse_event(&mut self, bytes: &[u8]) -> Result<ProviderFrame, TunnelError> {
        let provider_bytes = strict_event(bytes)?;
        let relay = Frame::decode(bytes).map_err(|_| TunnelError::Invalid)?;
        if relay.major != 1
            || relay.minor != 0
            || relay.seq != self.next_in
            || relay.worker_instance != self.worker
            || relay.device_epoch != self.epoch
        {
            return Err(TunnelError::Invalid);
        }
        let frame = self
            .provider_in
            .accept(provider_bytes)
            .map_err(|_| TunnelError::Invalid)?;
        if let Some(Payload::ProviderProgress(progress)) = &frame.payload
            && let Some(feedback) = &progress.feedback
            && (!self.capture_feedback_enabled
                || !self.capture_device.as_ref().is_some_and(|device| {
                    carosign_protocol::validation::capture_feedback_is_declared(feedback, device)
                }))
        {
            return Err(TunnelError::Invalid);
        }
        if let Some(Payload::ProviderProgress(progress)) = &frame.payload
            && let Some(state) = &progress.capture_state
            && (!self.capture_feedback_enabled
                || !self.capture_device.as_ref().is_some_and(|device| {
                    carosign_protocol::validation::capture_state_is_declared(state, device)
                }))
        {
            return Err(TunnelError::Invalid);
        }
        let is_hello = matches!(frame.payload, Some(Payload::WorkerHello(_)));
        let is_initialized = matches!(frame.payload, Some(Payload::Initialized(_)));
        if (!is_hello
            && !is_initialized
            && (frame.worker_instance_id != self.worker || frame.device_epoch != self.epoch))
            || (is_hello && frame.device_epoch != 0)
            || (is_initialized
                && (frame.worker_instance_id != self.worker || frame.device_epoch != 0))
        {
            return Err(TunnelError::Invalid);
        }
        if let Some(Payload::WorkerHello(hello)) = &frame.payload {
            carosign_protocol::validation::check_worker_hello(hello)
                .map_err(|_| TunnelError::Invalid)?;
        } else if let Some(Payload::AuthenticatorReady(ready)) = &frame.payload {
            if ready.authenticator.as_ref() != self.pending_authenticator.as_ref()
                || self.pending_authenticator.is_none()
                || ready.capabilities.is_none()
            {
                return Err(TunnelError::Invalid);
            }
            self.capture_device = ready.capabilities.clone();
            self.pending_authenticator = None;
        } else if let Some(Payload::PrebindDescription(description)) = &frame.payload {
            let Some((command, authenticator)) = self.pending_prebind.take() else {
                return Err(TunnelError::Invalid);
            };
            if frame.command_id != command
                || description.authenticator.as_ref() != Some(&authenticator)
                || description.declaration_version != 1
                || description.capabilities.is_none()
            {
                return Err(TunnelError::Invalid);
            }
        } else if let Some(Payload::EnrollmentChallengeReady(ready)) = &frame.payload {
            let Some((command, operation, mutation)) = self.pending_challenge.take() else {
                return Err(TunnelError::Invalid);
            };
            if frame.command_id != command
                || frame.operation_id != operation
                || ready.mutation_id != mutation
            {
                return Err(TunnelError::Invalid);
            }
            carosign_provider_sdk::events::parse(&frame).map_err(|_| TunnelError::Invalid)?;
        } else if let Some(Payload::CommandAck(ack)) = &frame.payload {
            carosign_provider_sdk::events::parse(&frame).map_err(|_| TunnelError::Invalid)?;
            if !ack.accepted
                && self
                    .pending_challenge
                    .as_ref()
                    .is_some_and(|(command, operation, _)| {
                        frame.command_id == *command && frame.operation_id == *operation
                    })
            {
                self.pending_challenge = None;
            }
        } else if !is_initialized {
            carosign_provider_sdk::events::parse(&frame).map_err(|_| TunnelError::Invalid)?;
        }
        self.next_in = self.next_in.checked_add(1).ok_or(TunnelError::Invalid)?;
        Ok(frame)
    }

    /// Sends a regular command and only accepts confirmation of the full send.
    /// Events prior to confirmation are verified and are limitedly stored.
    pub fn command(&mut self, frame: ProviderFrame, timeout: Duration) -> Result<(), TunnelError> {
        if matches!(frame.payload, Some(Payload::PrebindDescribe(_)))
            && self.pending_prebind.is_none()
        {
            self.close();
            return Err(TunnelError::Invalid);
        }
        if let Some(Payload::PrepareEnrollmentChallenge(request)) = frame.payload.as_ref() {
            if self.pending_challenge.is_some() {
                self.close();
                return Err(TunnelError::Invalid);
            }
            self.pending_challenge = Some((
                frame.command_id.clone(),
                frame.operation_id.clone(),
                request.mutation_id.clone(),
            ));
        }
        if let Some(Payload::Initialize(initialize)) = &frame.payload {
            self.capture_feedback_enabled = initialize
                .enabled_features
                .contains(&(Feature::CaptureFeedbackV1 as i32));
        }
        self.send_command(frame, None, timeout)
    }

    /// The declaration before Bind does not indicate readiness or right of access.
    pub fn prebind_describe(
        &mut self,
        frame: ProviderFrame,
        timeout: Duration,
    ) -> Result<ProviderFrame, TunnelError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(TunnelError::Timeout)?;
        let Some(Payload::PrebindDescribe(request)) = frame.payload.as_ref() else {
            self.close();
            return Err(TunnelError::Invalid);
        };
        let Some(authenticator) = request.authenticator.clone() else {
            self.close();
            return Err(TunnelError::Invalid);
        };
        if self.pending_prebind.is_some()
            || frame.command_id.len() != 16
            || frame.command_id.iter().all(|x| *x == 0)
            || !frame.operation_id.is_empty()
            || authenticator.device_epoch != self.epoch
        {
            self.close();
            return Err(TunnelError::Invalid);
        }
        self.pending_prebind = Some((frame.command_id.clone(), authenticator));
        self.command(frame, timeout)?;
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            self.close();
            return Err(TunnelError::Timeout);
        };
        let result = self.receive_event(remaining)?;
        if !matches!(result.payload, Some(Payload::PrebindDescription(_))) {
            self.close();
            return Err(TunnelError::Invalid);
        }
        Ok(result)
    }

    /// Transmits the ticket by value through a private channel. It can't be repeated.
    pub fn mutation(
        &mut self,
        frame: ProviderFrame,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        context: carosign_runtime::ports::MutationArmRequest,
        timeout: Duration,
    ) -> Result<(), TunnelError> {
        self.mutation_with_boundary(frame, ticket, context, timeout, &mut |send| send())
    }

    pub fn mutation_with_boundary(
        &mut self,
        frame: ProviderFrame,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        context: carosign_runtime::ports::MutationArmRequest,
        timeout: Duration,
        boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> Result<(), TunnelError> {
        if !context.matches_ticket(&ticket)
            || (carosign_adapters::provider::mutation_frame(&context)
                .ok()
                .as_ref()
                != Some(&frame)
                && carosign_adapters::provider::continuation_frame(&context)
                    .ok()
                    .as_ref()
                    != Some(&frame))
            || frame.operation_id != ticket.operation().as_bytes()
            || frame.command_id != ticket.command().as_bytes()
            || frame.worker_instance_id != ticket.worker().as_bytes()
            || frame.device_epoch != ticket.epoch().value()
        {
            self.close();
            return Err(TunnelError::Invalid);
        }
        let mutation = match frame.payload.as_ref() {
            Some(Payload::StartEnrollment(value)) => &value.mutation_id,
            Some(Payload::ContinueEnrollment(value)) => {
                &value
                    .enrollment
                    .as_ref()
                    .ok_or(TunnelError::Invalid)?
                    .mutation_id
            }
            Some(Payload::StartRemoval(value)) => &value.mutation_id,
            _ => {
                self.close();
                return Err(TunnelError::Invalid);
            }
        };
        if mutation.as_slice() != ticket.mutation().as_bytes() {
            self.close();
            return Err(TunnelError::Invalid);
        }
        let permit = MutationPermit {
            lease_id: context.lease.as_bytes().to_vec(),
            scope: ticket.scope().to_vec(),
            mutation_id: ticket.mutation().as_bytes().to_vec(),
        };
        self.send_command_with_boundary(frame, Some(permit), timeout, boundary)
    }

    fn send_command(
        &mut self,
        frame: ProviderFrame,
        permit: Option<MutationPermit>,
        timeout: Duration,
    ) -> Result<(), TunnelError> {
        self.send_command_with_boundary(frame, permit, timeout, &mut |send| send())
    }

    fn send_command_with_boundary(
        &mut self,
        frame: ProviderFrame,
        permit: Option<MutationPermit>,
        timeout: Duration,
        boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> Result<(), TunnelError> {
        if self.closed {
            return Err(TunnelError::Closed);
        }
        let result = (|| {
            let (bytes, request) = self
                .encoder
                .encode_command(frame, permit)
                .map_err(|_| TunnelError::Invalid)?;
            self.wait(libc::POLLOUT, timeout)?;
            let mut calls = 0u8;
            let mut sent = false;
            let permitted = boundary(&mut || {
                calls = calls.saturating_add(1);
                if calls != 1 {
                    return false;
                }
                sent = self.send_packet_now(&bytes).is_ok();
                sent
            });
            if !permitted || calls != 1 || !sent {
                return Err(TunnelError::Invalid);
            }
            let started = Instant::now();
            let reply = loop {
                let remaining = timeout
                    .checked_sub(started.elapsed())
                    .ok_or(TunnelError::Timeout)?;
                let reply = self.receive_packet(remaining)?;
                let decoded = Frame::decode(reply.as_slice()).map_err(|_| TunnelError::Invalid)?;
                if matches!(decoded.body, Some(Body::Event(_))) {
                    if self.pending_events.len() >= 64 {
                        return Err(TunnelError::Invalid);
                    }
                    let event = self.parse_event(&reply)?;
                    self.pending_events.push_back(event);
                    continue;
                }
                break reply;
            };
            strict_command_result(&reply)?;
            let decoded = Frame::decode(reply.as_slice()).map_err(|_| TunnelError::Invalid)?;
            if decoded.major != 1
                || decoded.minor != 0
                || decoded.seq != self.next_in
                || decoded.worker_instance != self.worker
                || decoded.device_epoch != self.epoch
            {
                return Err(TunnelError::Invalid);
            }
            let Some(Body::CommandResult(answer)) = decoded.body else {
                return Err(TunnelError::Invalid);
            };
            if answer.request_id != request
                || answer.result != Some(command_result::Result::Sent(true))
            {
                return Err(TunnelError::Invalid);
            }
            self.next_in = self.next_in.checked_add(1).ok_or(TunnelError::Invalid)?;
            Ok(())
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    /// Requests Bind from an broker without transmitting FD through a tunnel.
    /// Confirmation means local shipment, not device readiness.
    pub fn bind(
        &mut self,
        scope: Scope,
        slots: Vec<Slot>,
        frame: ProviderFrame,
        timeout: Duration,
    ) -> Result<(), TunnelError> {
        if self.closed {
            return Err(TunnelError::Closed);
        }
        let result = (|| {
            let Some(Payload::BindAuthenticator(bind)) = &frame.payload else {
                return Err(TunnelError::Invalid);
            };
            if self.pending_authenticator.is_some() || bind.authenticator.is_none() {
                return Err(TunnelError::Invalid);
            }
            self.pending_authenticator = bind.authenticator.clone();
            let (bytes, request) = self
                .encoder
                .bind(scope, slots, frame)
                .map_err(|_| TunnelError::Invalid)?;
            let started = Instant::now();
            self.send_packet(&bytes, timeout)?;
            loop {
                let remaining = timeout
                    .checked_sub(started.elapsed())
                    .ok_or(TunnelError::Timeout)?;
                let reply = self.receive_packet(remaining)?;
                let decoded = Frame::decode(reply.as_slice()).map_err(|_| TunnelError::Invalid)?;
                if matches!(decoded.body, Some(Body::Event(_))) {
                    if self.pending_events.len() >= 64 {
                        return Err(TunnelError::Invalid);
                    }
                    let event = self.parse_event(&reply)?;
                    self.pending_events.push_back(event);
                    continue;
                }
                strict_sent_result(&reply, 14)?;
                if decoded.major != 1
                    || decoded.minor != 0
                    || decoded.seq != self.next_in
                    || decoded.worker_instance != self.worker
                    || decoded.device_epoch != self.epoch
                {
                    return Err(TunnelError::Invalid);
                }
                let Some(Body::BindResult(answer)) = decoded.body else {
                    return Err(TunnelError::Invalid);
                };
                if answer.request_id != request
                    || answer.result != Some(bind_result::Result::Sent(true))
                {
                    return Err(TunnelError::Invalid);
                }
                self.next_in = self.next_in.checked_add(1).ok_or(TunnelError::Invalid)?;
                return Ok(());
            }
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    /// Transfers the area and a one-time cancellation identifier to the broker.
    /// The receipt confirms the dispatch of the command, not the stop of the provider.
    pub fn cancel_scoped(
        &mut self,
        scope: Scope,
        cancel_id: [u8; 16],
        timeout: Duration,
    ) -> Result<(), TunnelError> {
        self.control_scoped(scope, Some(cancel_id), timeout)
    }

    /// The review refers to one area and does not prove the rest of the device in itself.
    pub fn revoke_scoped(&mut self, scope: Scope, timeout: Duration) -> Result<(), TunnelError> {
        self.control_scoped(scope, None, timeout)
    }

    fn control_scoped(
        &mut self,
        scope: Scope,
        cancel_id: Option<[u8; 16]>,
        timeout: Duration,
    ) -> Result<(), TunnelError> {
        if self.closed {
            return Err(TunnelError::Closed);
        }
        let result = (|| {
            let (bytes, request) = self
                .encoder
                .control(scope, cancel_id)
                .map_err(|_| TunnelError::Invalid)?;
            self.send_packet(&bytes, timeout)?;
            let started = Instant::now();
            loop {
                let remaining = timeout
                    .checked_sub(started.elapsed())
                    .ok_or(TunnelError::Timeout)?;
                let reply = self.receive_packet(remaining)?;
                let decoded = Frame::decode(reply.as_slice()).map_err(|_| TunnelError::Invalid)?;
                if matches!(decoded.body, Some(Body::Event(_))) {
                    if self.pending_events.len() >= 64 {
                        return Err(TunnelError::Invalid);
                    }
                    let event = self.parse_event(&reply)?;
                    self.pending_events.push_back(event);
                    continue;
                }
                strict_result(&reply, 16, if cancel_id.is_some() { 2 } else { 3 })?;
                if decoded.major != 1
                    || decoded.minor != 0
                    || decoded.seq != self.next_in
                    || decoded.worker_instance != self.worker
                    || decoded.device_epoch != self.epoch
                {
                    return Err(TunnelError::Invalid);
                }
                let Some(Body::ControlResult(answer)) = decoded.body else {
                    return Err(TunnelError::Invalid);
                };
                let expected = if cancel_id.is_some() {
                    control_result::Result::CancelSent(true)
                } else {
                    control_result::Result::Revoked(true)
                };
                if answer.request_id != request || answer.result != Some(expected) {
                    return Err(TunnelError::Invalid);
                }
                self.next_in = self.next_in.checked_add(1).ok_or(TunnelError::Invalid)?;
                return Ok(());
            }
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    pub(super) fn has_event(&self, timeout: Duration) -> Result<bool, TunnelError> {
        if self.closed {
            return Err(TunnelError::Closed);
        }
        if !self.pending_events.is_empty() {
            return Ok(true);
        }
        match self.wait(libc::POLLIN, timeout) {
            Ok(()) => Ok(true),
            Err(TunnelError::Timeout) => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn wait(&self, events: libc::c_short, timeout: Duration) -> Result<(), TunnelError> {
        let millis = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        let mut poll = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events,
            revents: 0,
        };
        // SAFETY: The poll indicates one active element.
        let rc = unsafe { libc::poll(&mut poll, 1, millis) };
        if rc == 0 {
            return Err(TunnelError::Timeout);
        }
        if rc < 0 {
            return Err(TunnelError::Io(std::io::Error::last_os_error()));
        }
        if poll.revents & events == 0 {
            return Err(TunnelError::Closed);
        }
        Ok(())
    }

    fn send_packet(&self, data: &[u8], timeout: Duration) -> Result<(), TunnelError> {
        self.wait(libc::POLLOUT, timeout)?;
        self.send_packet_now(data)
    }

    fn send_packet_now(&self, data: &[u8]) -> Result<(), TunnelError> {
        // SAFETY: The buffer and descriptor live until the send return.
        let n = unsafe {
            libc::send(
                self.fd.as_raw_fd(),
                data.as_ptr().cast(),
                data.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if n < 0 {
            return Err(TunnelError::Io(std::io::Error::last_os_error()));
        }
        if n as usize != data.len() {
            return Err(TunnelError::Invalid);
        }
        Ok(())
    }

    fn receive_packet(&self, timeout: Duration) -> Result<Zeroizing<Vec<u8>>, TunnelError> {
        self.wait(libc::POLLIN, timeout)?;
        let mut data = Zeroizing::new(vec![0; carosign_protocol::limits::FRAME_MAX]);
        let mut iov = libc::iovec {
            iov_base: data.as_mut_ptr().cast(),
            iov_len: data.len(),
        };
        // SAFETY: A zero-initialized `msghdr` is valid here. Without a control buffer, the kernel
        // closes extra `SCM_RIGHTS` descriptors, and this code rejects `MSG_CTRUNC`.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut iov;
        message.msg_iovlen = 1;
        let n = unsafe {
            libc::recvmsg(
                self.fd.as_raw_fd(),
                &mut message,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if n < 0 {
            return Err(TunnelError::Io(std::io::Error::last_os_error()));
        }
        if n == 0 {
            return Err(TunnelError::Closed);
        }
        if message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
            return Err(TunnelError::Invalid);
        }
        data.truncate(n as usize);
        Ok(data)
    }

    fn close(&mut self) {
        self.closed = true;
        // SAFETY: The socket belongs to the object, the shutdown only stops the exchange.
        unsafe {
            libc::shutdown(self.fd.as_raw_fd(), libc::SHUT_RDWR);
        }
    }
}

#[path = "tunnel_client.tests.rs"]
#[cfg(test)]
mod tests;

#[path = "tunnel_client.mutation_encoder_tests.tests.rs"]
#[cfg(test)]
mod mutation_encoder_tests;
