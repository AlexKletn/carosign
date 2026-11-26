use super::{ConnectionState, TransportError};

use carosign_protocol::{
    carosign::client::v1::{self as wire, client_frame::Payload},
    limits, validation,
};
use prost::Message;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// It establishes a connection and coordinates a version; the identity of the service is not
/// confirmed by this, and trusted operations require separate environmental verification.
pub struct Transport {
    fd: OwnedFd,
    minor: u32,
    capture_feedback_v1: bool,
    verification_enrollment_selector: bool,
    state: std::sync::Mutex<ConnectionState>,
    request_lock: std::sync::Mutex<()>,
    receive_lock: std::sync::Mutex<()>,
}

// The upper limit of the queue for events that occurred during another exchange.
// When overflowing, the connection is closed: the terminal cannot be lost silently.
const PENDING_EVENTS_MAX: usize = 64;

impl Transport {
    /// The caller must establish trust in the service.
    pub fn connect(path: &str) -> Result<Self, TransportError> {
        Self::connect_with_peer(path, None)
    }

    /// Connects the installed system service and checks its UID before sending any data,
    /// trusting the system root is assumed.
    pub fn connect_root(path: &str) -> Result<Self, TransportError> {
        Self::connect_with_peer(path, Some(0))
    }

    fn connect_with_peer(
        path: &str,
        expected_uid: Option<libc::uid_t>,
    ) -> Result<Self, TransportError> {
        if !std::path::Path::new(path).is_absolute() || path.as_bytes().contains(&0) {
            return Err(TransportError::Unavailable);
        }
        // SAFETY: The new descriptor is transmitted to the owner exactly once.
        let raw = unsafe {
            libc::socket(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
                0,
            )
        };
        if raw < 0 {
            return Err(TransportError::Unavailable);
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let mut address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let path = path.as_bytes();
        if path.is_empty() || path.len() >= address.sun_path.len() {
            return Err(TransportError::Unavailable);
        }
        for (dst, src) in address.sun_path.iter_mut().zip(path) {
            *dst = *src as libc::c_char;
        }
        let len = (core::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + 1)
            as libc::socklen_t;
        // SAFETY: The address is filled and the entire system call is valid.
        if unsafe {
            libc::connect(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                len,
            )
        } != 0
        {
            // Crowded queue AF_UNIX And the incomplete connection must not delay the flow of
            // the input: the permission is not yet, we refuse.
            return Err(TransportError::Unavailable);
        }
        if let Some(uid) = expected_uid {
            require_peer_uid(&fd, uid)?;
        }
        // The following operations are limited SO_RCVTIMEO/SO_SNDTIMEOIt's not enough to
        // install them for a non-blocking socket, so we only return it to normal mode after a
        // successful connect.
        // SAFETY: The descriptor belongs to this call and has not yet been transmitted
        // outward.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK) } < 0
        {
            return Err(TransportError::Unavailable);
        }
        Self::from_connected(fd)
    }

    /// Uses an already connected socket; the origin checks the caller.
    pub fn from_connected(fd: OwnedFd) -> Result<Self, TransportError> {
        for (option, expected) in [
            (libc::SO_TYPE, libc::SOCK_SEQPACKET),
            (libc::SO_DOMAIN, libc::AF_UNIX),
        ] {
            let mut actual: libc::c_int = 0;
            let mut len = core::mem::size_of_val(&actual) as libc::socklen_t;
            // SAFETY: Values and size are valid until getockopt returns.
            if unsafe {
                libc::getsockopt(
                    fd.as_raw_fd(),
                    libc::SOL_SOCKET,
                    option,
                    (&mut actual as *mut libc::c_int).cast(),
                    &mut len,
                )
            } != 0
                || len as usize != core::mem::size_of_val(&actual)
                || actual != expected
            {
                return Err(TransportError::Unavailable);
            }
        }
        let timeout = libc::timeval {
            tv_sec: 5,
            tv_usec: 0,
        };
        for option in [libc::SO_RCVTIMEO, libc::SO_SNDTIMEO] {
            // SAFETY: Correct size and timeval address.
            if unsafe {
                libc::setsockopt(
                    fd.as_raw_fd(),
                    libc::SOL_SOCKET,
                    option,
                    (&timeout as *const libc::timeval).cast(),
                    core::mem::size_of_val(&timeout) as libc::socklen_t,
                )
            } != 0
            {
                return Err(TransportError::Unavailable);
            }
        }
        let id = random_id()?;
        send(
            &fd,
            &frame(
                &id,
                limits::MINOR_BASELINE,
                Payload::Hello(wire::Hello {
                    capture_feedback_v1: true,
                    major: limits::MAJOR,
                    min_minor: limits::MINOR_BASELINE,
                    max_minor: limits::MINOR_MAX,
                }),
            ),
        )?;
        let answer = receive(&fd, &id)?;
        let Some(Payload::HelloAck(ack)) = answer.payload else {
            return Err(TransportError::Unexpected);
        };
        if !answer.operation_id.is_empty()
            || answer.event_sequence != 0
            || ack.major != limits::MAJOR
            || ack.selected_minor < limits::MINOR_BASELINE
            || ack.selected_minor > limits::MINOR_MAX
            || answer.minor != ack.selected_minor
            || ack.connection_id.len() != 16
            || ack.connection_id.iter().all(|v| *v == 0)
        {
            return Err(TransportError::Unexpected);
        }
        Ok(Self {
            fd,
            minor: ack.selected_minor,
            capture_feedback_v1: ack.capture_feedback_v1,
            verification_enrollment_selector: ack.verification_enrollment_selector,
            state: std::sync::Mutex::new(ConnectionState {
                closed: false,
                events: std::collections::VecDeque::new(),
                pending_request: None,
                response: None,
            }),
            request_lock: std::sync::Mutex::new(()),
            receive_lock: std::sync::Mutex::new(()),
        })
    }

    /// Sends one request and checks the relationship of the response to it.
    pub fn supports_verification_enrollment_selector(&self) -> bool {
        self.verification_enrollment_selector
    }

    pub fn request(&self, payload: Payload) -> Result<wire::ClientFrame, TransportError> {
        if matches!(&payload, Payload::BeginVerification(request) if request.enrollment.is_some() || request.report_matched_enrollment)
            && !self.supports_verification_enrollment_selector()
        {
            return Err(TransportError::Unexpected);
        }
        // Requests remain consistent, but waiting for an event does not keep that lock and
        // does not prevent you from sending a cancellation.
        let _request = self
            .request_lock
            .lock()
            .map_err(|_| TransportError::Unavailable)?;
        let id = random_id()?;
        let begin_operation = matches!(
            &payload,
            Payload::BeginVerification(_) | Payload::BeginEnrollment(_) | Payload::BeginRemoval(_)
        );
        let cancel_operation = matches!(&payload, Payload::CancelOperation(_));
        let enrollment = matches!(&payload, Payload::BeginEnrollment(_));
        let request = frame(&id, self.minor, payload);
        validation::check_client_frame(&request).map_err(|_| TransportError::Unexpected)?;
        {
            let mut state = self.state.lock().map_err(|_| TransportError::Unavailable)?;
            if state.closed || state.pending_request.is_some() || state.response.is_some() {
                return Err(TransportError::Unavailable);
            }
            state.pending_request = Some(id);
        }
        let deadline = if enrollment {
            std::time::Instant::now() + std::time::Duration::from_secs(60)
        } else {
            self.deadline()
        };
        let response = send(&self.fd, &request)
            .and_then(|()| {
                loop {
                    {
                        let mut state =
                            self.state.lock().map_err(|_| TransportError::Unavailable)?;
                        if state.closed {
                            return Err(TransportError::Unavailable);
                        }
                        if let Some(answer) = state.response.take() {
                            state.pending_request = None;
                            break Ok(answer);
                        }
                    }
                    self.pump(deadline, None)?;
                }
            })
            .and_then(|answer| {
                if answer.minor != self.minor || answer.event_sequence != 0 {
                    return Err(TransportError::Unexpected);
                }
                match &answer.payload {
                    Some(Payload::OperationAccepted(accepted)) if begin_operation => {
                        let valid_id = accepted.operation_id.len() == 16
                            && accepted.operation_id.iter().any(|byte| *byte != 0)
                            && accepted.operation_id == answer.operation_id;
                        let valid_deadline = accepted.deadline.as_ref().is_some_and(|deadline| {
                            deadline.boot_id.len() == 16
                                && deadline.boot_id.iter().any(|byte| *byte != 0)
                                && deadline.boottime_ns != 0
                        });
                        if !valid_id || !valid_deadline || accepted.first_event_sequence == 0 {
                            return Err(TransportError::Unexpected);
                        }
                    }
                    Some(Payload::ProtocolError(_)) if answer.operation_id.is_empty() => {}
                    Some(Payload::CancellationReceipt(receipt))
                        if cancel_operation
                            && answer.operation_id.is_empty()
                            && [
                                wire::CancelReceiptKind::CancelReceiptRequested as i32,
                                wire::CancelReceiptKind::CancelReceiptAlreadyTerminal as i32,
                            ]
                            .contains(&receipt.result) => {}
                    _ if !begin_operation
                        && !cancel_operation
                        && answer.operation_id.is_empty() => {}
                    _ => return Err(TransportError::Unexpected),
                }
                Ok(answer)
            });
        if response.is_err() {
            // After an unsuccessful exchange, the answer may come later, and the socket can
            // no longer be used for repeating or for a new request.
            self.close();
        }
        response
    }

    /// Accepts the following event of a previously accepted audit. The event does not confirm
    /// the credibility of the service or the authority of the caller.
    pub fn receive_verification_event(
        &self,
        tracker: &mut super::super::verification_events::VerificationEvents,
    ) -> Result<(wire::ClientFrame, bool), TransportError> {
        self.receive_verification_event_until(tracker, self.deadline())
    }

    /// Accepts an event up to the total duration of the operation, not up to the duration of
    /// the individual RPC.
    pub fn receive_verification_event_until(
        &self,
        tracker: &mut super::super::verification_events::VerificationEvents,
        deadline: std::time::Instant,
    ) -> Result<(wire::ClientFrame, bool), TransportError> {
        let result = (|| {
            let frame = self.receive_operation_event_until(tracker.operation_id(), deadline)?;
            let terminal = tracker
                .accept(&frame)
                .map_err(|_| TransportError::Unexpected)?;
            Ok((frame, terminal))
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    /// Returns the next frame of the operation; order and view checks are performed by the
    /// appropriate event controller. The error closes the connection.
    pub fn receive_operation_event_until(
        &self,
        operation: &[u8],
        deadline: std::time::Instant,
    ) -> Result<wire::ClientFrame, TransportError> {
        let result = (|| loop {
            let mut state = self.state.lock().map_err(|_| TransportError::Unavailable)?;
            if state.closed {
                return Err(TransportError::Unavailable);
            }
            if let Some(index) = state
                .events
                .iter()
                .position(|frame| frame.operation_id == operation)
            {
                return state.events.remove(index).ok_or(TransportError::Unexpected);
            }
            drop(state);
            self.pump(deadline, Some(operation))?;
        })();
        if result.is_err() {
            self.close();
        }
        result
    }

    fn deadline(&self) -> std::time::Instant {
        let mut timeout: libc::timeval = unsafe { core::mem::zeroed() };
        let mut len = core::mem::size_of_val(&timeout) as libc::socklen_t;
        // SAFETY: Timeout and len are valid for the duration of the call.
        let ok = unsafe {
            libc::getsockopt(
                self.fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&mut timeout as *mut libc::timeval).cast(),
                &mut len,
            )
        } == 0;
        let duration = if ok && timeout.tv_sec >= 0 && timeout.tv_usec >= 0 {
            std::time::Duration::from_secs(timeout.tv_sec as u64)
                + std::time::Duration::from_micros(timeout.tv_usec as u64)
        } else {
            std::time::Duration::from_secs(5)
        };
        std::time::Instant::now()
            + if duration.is_zero() {
                std::time::Duration::from_secs(5)
            } else {
                duration
            }
    }

    // The reader is selected for a short period of time: another stream can send a
    // cancellation, and the resulting packet is routed to its waiting owner.
    fn pump(
        &self,
        deadline: std::time::Instant,
        operation: Option<&[u8]>,
    ) -> Result<(), TransportError> {
        let _reader = self
            .receive_lock
            .lock()
            .map_err(|_| TransportError::Unavailable)?;
        {
            let state = self.state.lock().map_err(|_| TransportError::Unavailable)?;
            if state.closed {
                return Err(TransportError::Unavailable);
            }
            let ready = match operation {
                None => state.response.is_some(),
                Some(id) => state.events.iter().any(|frame| frame.operation_id == id),
            };
            if ready {
                return Ok(());
            }
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(TransportError::Unavailable);
        }
        let mut ready = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let wait_ms = remaining
            .min(std::time::Duration::from_millis(20))
            .as_millis()
            .max(1) as i32;
        // SAFETY: pollfd is valid during the call.
        let result = unsafe { libc::poll(&mut ready, 1, wait_ms) };
        if result < 0 {
            return Err(TransportError::Unavailable);
        }
        if result == 0 {
            return Ok(());
        }
        let frame = receive_packet(&self.fd)?;
        let mut state = self.state.lock().map_err(|_| TransportError::Unavailable)?;
        if state.closed {
            return Err(TransportError::Unavailable);
        }
        if frame.request_id.is_empty() {
            queue_event(&mut state, frame, self.minor, self.capture_feedback_v1)
        } else {
            validation::check_client_frame(&frame).map_err(|_| TransportError::Unexpected)?;
            if state.pending_request.as_ref().map(|id| id.as_slice())
                != Some(frame.request_id.as_slice())
                || state.response.is_some()
            {
                return Err(TransportError::Unexpected);
            }
            state.response = Some(frame);
            Ok(())
        }
    }

    pub(crate) fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.events.clear();
            state.response = None;
            state.pending_request = None;
        }
        // SAFETY: The socket belongs to live transport.
        unsafe { libc::shutdown(self.fd.as_raw_fd(), libc::SHUT_RDWR) };
    }
}

fn require_peer_uid(fd: &OwnedFd, expected_uid: libc::uid_t) -> Result<(), TransportError> {
    // SAFETY: The kernel writes to a structure of the correct size.
    let mut peer: libc::ucred = unsafe { core::mem::zeroed() };
    let mut len = core::mem::size_of_val(&peer) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut len,
        )
    } != 0
        || len as usize != core::mem::size_of_val(&peer)
        || peer.uid != expected_uid
        || peer.pid <= 0
    {
        return Err(TransportError::Unavailable);
    }
    Ok(())
}

fn check_event_envelope(frame: &wire::ClientFrame, minor: u32) -> Result<(), TransportError> {
    let Some(Payload::OperationEvent(event)) = &frame.payload else {
        return Err(TransportError::Unexpected);
    };
    if frame.major != limits::MAJOR
        || frame.minor != minor
        || !frame.request_id.is_empty()
        || frame.operation_id.len() != limits::ID_LEN
        || frame.operation_id.iter().all(|byte| *byte == 0)
        || frame.operation_id != event.operation_id
        || frame.event_sequence == 0
        || frame.event_sequence != event.sequence
    {
        return Err(TransportError::Unexpected);
    }
    Ok(())
}

fn queue_event(
    state: &mut ConnectionState,
    mut frame: wire::ClientFrame,
    minor: u32,
    capture_feedback_v1: bool,
) -> Result<(), TransportError> {
    check_event_envelope(&frame, minor)?;
    if state.events.len() >= PENDING_EVENTS_MAX {
        return Err(TransportError::Unexpected);
    }
    if let Some(Payload::OperationEvent(event)) = &mut frame.payload {
        super::capture_feedback::normalize(event, capture_feedback_v1)?;
    }
    state.events.push_back(frame);
    Ok(())
}

fn random_id() -> Result<[u8; 16], TransportError> {
    let value = carosign_linux::sys::random_16().map_err(|_| TransportError::Unavailable)?;
    if value.iter().all(|v| *v == 0) {
        return Err(TransportError::Unavailable);
    }
    Ok(value)
}
fn frame(id: &[u8; 16], minor: u32, payload: Payload) -> wire::ClientFrame {
    wire::ClientFrame {
        major: limits::MAJOR,
        minor,
        request_id: id.to_vec(),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(payload),
    }
}
fn send(fd: &OwnedFd, frame: &wire::ClientFrame) -> Result<(), TransportError> {
    let bytes = frame.encode_to_vec();
    if bytes.is_empty() || bytes.len() > limits::FRAME_MAX {
        return Err(TransportError::Unexpected);
    }
    // SAFETY: The buffer is valid for call time, SIGPIPE is suppressed.
    let sent = unsafe {
        libc::send(
            fd.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    if sent != bytes.len() as isize {
        return Err(TransportError::Unavailable);
    }
    Ok(())
}
fn receive(fd: &OwnedFd, request_id: &[u8; 16]) -> Result<wire::ClientFrame, TransportError> {
    let frame = receive_packet(fd)?;
    validation::check_client_frame(&frame).map_err(|_| TransportError::Unexpected)?;
    if frame.request_id != request_id {
        return Err(TransportError::Unexpected);
    }
    Ok(frame)
}

fn receive_packet(fd: &OwnedFd) -> Result<wire::ClientFrame, TransportError> {
    let mut bytes = vec![0; limits::FRAME_MAX];
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut message: libc::msghdr = unsafe { core::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    // SAFETY: The kernel closes the transmitted FDs without a control buffer.
    let read = unsafe { libc::recvmsg(fd.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
    if read <= 0 {
        return Err(TransportError::Unavailable);
    }
    if message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
        return Err(TransportError::Unexpected);
    }
    let bytes = &bytes[..read as usize];
    check_one_payload(bytes)?;
    let frame = wire::ClientFrame::decode(bytes).map_err(|_| TransportError::Unexpected)?;
    Ok(frame)
}
fn check_one_payload(mut bytes: &[u8]) -> Result<(), TransportError> {
    use prost::encoding::{DecodeContext, decode_key, skip_field};
    let mut found = false;
    while !bytes.is_empty() {
        let (tag, wire_type) = decode_key(&mut bytes).map_err(|_| TransportError::Unexpected)?;
        if matches!(tag, 10..=17 | 20..=33 | 90) {
            if found {
                return Err(TransportError::Unexpected);
            }
            found = true;
        }
        skip_field(wire_type, tag, &mut bytes, DecodeContext::default())
            .map_err(|_| TransportError::Unexpected)?;
    }
    if !found {
        return Err(TransportError::Unexpected);
    }
    Ok(())
}

#[path = "client_transport.tests.rs"]
#[cfg(test)]
mod tests;
