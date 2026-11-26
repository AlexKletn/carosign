use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

use carosign_core::authorization::ValidatedGrant;
use carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope;
use prost::Message;

use super::message::{
    Authorization, Bootstrap, GrantBinding, Request, Response, request, response,
};
use super::peer::{ExpectedPeer, check_peer};
use super::scope::{canonical_scope, parse_scope};
use super::validation::{response_matches, valid_action};

const MAX_PACKET: usize = 2048;
const VERSION: u32 = 1;

/// The connection is tied to a single context; the sequences are not repeated.
pub struct AuthorityChannel {
    fd: OwnedFd,
    scope: Vec<u8>,
    next: u64,
    failed: bool,
}

impl AuthorityChannel {
    /// Only to wait for readiness; the reading stays behind the channel.
    pub fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.fd.as_fd()
    }

    /// Transmits the initial area through the owner’s already protected channel.
    /// Confirmation means only acceptance of the form, not admission or credential.
    pub fn bootstrap_client(
        fd: OwnedFd,
        peer: ExpectedPeer,
        scope: &EnrollmentAuthorizationScope,
        grant: GrantBinding,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> io::Result<Self> {
        if !grant.valid() || payload.len() > 1024 {
            return Err(io::Error::other("неверный bootstrap"));
        }
        let mut channel = Self::new(fd, peer, scope)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::other("срок"))?;
        let bootstrap = Bootstrap {
            version: VERSION,
            scope: canonical_scope(scope),
            grant: Some(grant),
            payload,
        };
        let result = (|| {
            channel.send(&bootstrap.encode_to_vec(), deadline)?;
            let ack = channel.receive(deadline)?;
            if ack != [1] {
                return Err(io::Error::other("неверный ответ bootstrap"));
            }
            Ok(())
        })();
        if result.is_err() {
            channel.fail();
        }
        result.map(|_| channel)
    }

    /// Receives bootstrap from a trusted launcher.Returned data is subject to independent
    /// verification by the owner before any hardware calls.
    pub fn bootstrap_server(
        fd: OwnedFd,
        peer: ExpectedPeer,
        timeout: Duration,
    ) -> io::Result<(Self, EnrollmentAuthorizationScope, GrantBinding, Vec<u8>)> {
        check_peer(&fd, peer)?;
        let mut channel = Self {
            fd,
            scope: Vec::new(),
            next: 1,
            failed: false,
        };
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::other("срок"))?;
        let result = (|| {
            let raw = channel.receive(deadline)?;
            let bootstrap = Bootstrap::decode(raw.as_slice())
                .map_err(|_| io::Error::other("неверный bootstrap"))?;
            if bootstrap.encode_to_vec() != raw
                || bootstrap.version != VERSION
                || bootstrap.payload.len() > 1024
            {
                return Err(io::Error::other("неверный bootstrap"));
            }
            let scope = parse_scope(&bootstrap.scope)?;
            let grant = bootstrap
                .grant
                .filter(GrantBinding::valid)
                .ok_or_else(|| io::Error::other("неверный grant"))?;
            channel.scope = bootstrap.scope;
            channel.send(&[1], deadline)?;
            Ok((scope, grant, bootstrap.payload))
        })();
        match result {
            Ok((scope, grant, payload)) => Ok((channel, scope, grant, payload)),
            Err(error) => {
                channel.fail();
                Err(error)
            }
        }
    }

    /// Checks the stable ticket and calls the recipient, holding the total clearance until
    /// the channel responds. `check_live`.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_admitted(
        &mut self,
        gate: &carosign_runtime::enrollment_admission::SerializedEnrollmentAdmission,
        scope: &EnrollmentAuthorizationScope,
        authorization: &carosign_runtime::enrollment_authorization::AuthorizedEnrollment,
        grant: &ValidatedGrant,
        clock: &dyn carosign_runtime::ports::TimeSource,
        store: &dyn carosign_persistence::store::MetadataStore,
        request: &carosign_runtime::ports::MutationArmRequest,
        ticket: carosign_persistence::journal::ArmedMutationTicket,
        timeout: Duration,
        check_live: impl FnOnce() -> bool,
        continuation: impl FnOnce(
            carosign_persistence::journal::ArmedMutationTicket,
        )
            -> carosign_runtime::enrollment_admission::ConsumptionOutcome,
    ) -> Result<
        carosign_runtime::enrollment_admission::ConsumptionOutcome,
        carosign_runtime::enrollment_admission::AdmissionError,
    > {
        if !self.belongs_to(scope) {
            return Err(carosign_runtime::enrollment_admission::AdmissionError::Invalid);
        }
        gate.consume(
            scope,
            authorization,
            grant,
            clock,
            store,
            request,
            ticket,
            check_live,
            |ticket| {
                let outcome = self.execute(
                    GrantBinding::from_grant(grant),
                    *authorization.reference(),
                    *authorization.digest(),
                    timeout,
                );
                if outcome != carosign_runtime::enrollment_admission::ConsumptionOutcome::Completed
                {
                    return outcome;
                }
                if gate.revocation_requested() {
                    return carosign_runtime::enrollment_admission::ConsumptionOutcome::Unknown;
                }
                continuation(ticket)
            },
        )
    }

    /// The synchronous boundary of the trusted recipient. `Accepted` allow only after the
    /// completion of the action, `Refused` A transport error does not prove that the
    /// recipient did not initiate the action.
    /// The caller is obliged to hold the clearance and live issuance until the return and
    /// maintain the fence in the event of an uncertain outcome.
    pub fn execute(
        &mut self,
        grant: GrantBinding,
        reference: [u8; 16],
        digest: [u8; 32],
        timeout: Duration,
    ) -> carosign_runtime::enrollment_admission::ConsumptionOutcome {
        use carosign_runtime::enrollment_admission::ConsumptionOutcome;
        match self.request(
            grant,
            request::Action::Execute(Authorization {
                reference: reference.to_vec(),
                challenge_digest: digest.to_vec(),
            }),
            timeout,
        ) {
            Ok(response::Result::Accepted(_)) => ConsumptionOutcome::Completed,
            Ok(response::Result::Refused(_)) => ConsumptionOutcome::NotDispatched,
            _ => ConsumptionOutcome::Unknown,
        }
    }

    pub fn belongs_to(&self, scope: &EnrollmentAuthorizationScope) -> bool {
        self.scope == canonical_scope(scope) && !self.failed
    }
    /// Descriptor and expected_peer It gives out a trusted launcher, scope, from an already
    /// authorized operation, peer verification is not a substitute for authority
    /// verification.
    pub fn new(
        fd: OwnedFd,
        peer: ExpectedPeer,
        scope: &EnrollmentAuthorizationScope,
    ) -> io::Result<Self> {
        check_peer(&fd, peer)?;
        Ok(Self {
            fd,
            scope: canonical_scope(scope),
            next: 1,
            failed: false,
        })
    }

    /// The client performs one limited exchange. Any ambiguity closes the channel; a repeat
    /// on another channel is not a permitted repeat of the operation.
    pub fn request(
        &mut self,
        grant: GrantBinding,
        action: request::Action,
        timeout: Duration,
    ) -> io::Result<response::Result> {
        self.request_with_boundary(grant, action, timeout, &mut |send| send())
    }

    /// The border holds the verified lease only for the duration of the non-blocking send.
    /// The expectation of readiness and response takes place outside of it.
    pub fn request_with_boundary(
        &mut self,
        grant: GrantBinding,
        action: request::Action,
        timeout: Duration,
        boundary: &mut dyn FnMut(&mut dyn FnMut() -> bool) -> bool,
    ) -> io::Result<response::Result> {
        if self.failed || !grant.valid() || !valid_action(&action) {
            return Err(io::Error::other("неверный запрос"));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "срок"))?;
        let req = Request {
            version: VERSION,
            sequence: self.next,
            scope: self.scope.clone(),
            grant: Some(grant),
            action: Some(action.clone()),
        };
        let result = (|| {
            let bytes = req.encode_to_vec();
            if bytes.is_empty() || bytes.len() > MAX_PACKET {
                return Err(io::Error::other("размер пакета"));
            }
            self.wait(libc::POLLOUT, deadline)?;
            let mut calls = 0u8;
            let mut send_ok = false;
            let sent = boundary(&mut || {
                calls = calls.saturating_add(1);
                if calls != 1 {
                    return false;
                }
                send_ok = self.send_now(&bytes).is_ok();
                send_ok
            });
            if !sent || calls != 1 || !send_ok {
                return Err(io::Error::other("отправка не допущена"));
            }
            let raw = self.receive(deadline)?;
            let response =
                Response::decode(raw.as_slice()).map_err(|_| io::Error::other("неверный ответ"))?;
            if response.encode_to_vec() != raw
                || response.version != VERSION
                || response.sequence != self.next
                || !response.result.as_ref().is_some_and(|r| {
                    response_matches(
                        &action,
                        r,
                        req.grant.as_ref().expect("проверено").deadline_ns,
                    )
                })
            {
                return Err(io::Error::other("несогласованный ответ"));
            }
            self.next = self
                .next
                .checked_add(1)
                .ok_or_else(|| io::Error::other("исчерпана последовательность"))?;
            Ok(response.result.expect("проверено"))
        })();
        if result.is_err() {
            self.fail();
        }
        result
    }

    /// Handler is required to verify grant, lease, review, trusted challenge session and
    /// recipient; this method only checks the shape, link and correlation of the result.
    pub fn serve_one(
        &mut self,
        timeout: Duration,
        handler: impl FnOnce(&GrantBinding, &request::Action) -> response::Result,
    ) -> io::Result<()> {
        self.serve_one_fallible(timeout, |grant, action| Ok(handler(grant, action)))
    }

    /// The handler error closes the channel without a response: the result of the action that
    /// has started remains unknown, and does not turn into a failure before the call.
    pub fn serve_one_fallible(
        &mut self,
        timeout: Duration,
        handler: impl FnOnce(&GrantBinding, &request::Action) -> io::Result<response::Result>,
    ) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("канал закрыт"));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "срок"))?;
        let result = (|| {
            let raw = self.receive(deadline)?;
            let request =
                Request::decode(raw.as_slice()).map_err(|_| io::Error::other("неверный запрос"))?;
            if request.encode_to_vec() != raw
                || request.version != VERSION
                || request.sequence != self.next
                || request.scope != self.scope
                || !request.grant.as_ref().is_some_and(GrantBinding::valid)
                || !request.action.as_ref().is_some_and(valid_action)
            {
                return Err(io::Error::other("несогласованный запрос"));
            }
            let action = request.action.as_ref().expect("проверено");
            let response = handler(request.grant.as_ref().expect("проверено"), action)?;
            if !response_matches(
                action,
                &response,
                request.grant.as_ref().expect("проверено").deadline_ns,
            ) {
                return Err(io::Error::other("несогласованный ответ"));
            }
            self.send(
                &Response {
                    version: VERSION,
                    sequence: self.next,
                    result: Some(response),
                }
                .encode_to_vec(),
                deadline,
            )?;
            self.next = self
                .next
                .checked_add(1)
                .ok_or_else(|| io::Error::other("исчерпана последовательность"))?;
            Ok(())
        })();
        if result.is_err() {
            self.fail();
        }
        result
    }

    fn wait(&self, events: libc::c_short, deadline: Instant) -> io::Result<()> {
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "срок"))?;
            let mut pollfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events,
                revents: 0,
            };
            let millis = remaining
                .as_millis()
                .saturating_add(1)
                .min(i32::MAX as u128) as i32;
            // SAFETY: The `pollfd` is valid, and the call receives one entry.
            let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
            if result > 0 {
                return Ok(());
            }
            if result == 0 {
                continue;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    fn send(&self, bytes: &[u8], deadline: Instant) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_PACKET {
            return Err(io::Error::other("размер пакета"));
        }
        self.wait(libc::POLLOUT, deadline)?;
        self.send_now(bytes)
    }

    fn send_now(&self, bytes: &[u8]) -> io::Result<()> {
        // SAFETY: fd and buffer are valid, the sending does not block the flow.
        let sent = unsafe {
            libc::send(
                self.fd.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent < 0 {
            return Err(io::Error::last_os_error());
        }
        if sent as usize != bytes.len() {
            return Err(io::Error::other("неполная отправка"));
        }
        Ok(())
    }

    fn receive(&self, deadline: Instant) -> io::Result<Vec<u8>> {
        self.wait(libc::POLLIN, deadline)?;
        let mut bytes = vec![0; MAX_PACKET];
        let mut iov = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: Zero msghdr is acceptable; ancillary data is not accepted.
        let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        // SAFETY: The `fd` and `iov` are valid. Without a control buffer, the kernel closes
        // descriptors received via `SCM_RIGHTS` and reports truncation via `MSG_CTRUNC`.
        let count = unsafe {
            libc::recvmsg(
                self.fd.as_raw_fd(),
                &mut msg,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        if count == 0 || msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
            return Err(io::Error::other("обрезанный или пустой пакет"));
        }
        bytes.truncate(count as usize);
        Ok(bytes)
    }

    fn fail(&mut self) {
        self.failed = true;
        // SAFETY: fd belongs to the object; shutdown closes both directions.
        unsafe {
            libc::shutdown(self.fd.as_raw_fd(), libc::SHUT_RDWR);
        }
    }
}

#[path = "channel.tests.rs"]
#[cfg(test)]
mod tests;
