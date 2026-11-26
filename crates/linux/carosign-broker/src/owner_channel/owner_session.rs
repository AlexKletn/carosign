use super::Reply;

use carosign_protocol::{
    carosign::resource_owner::v1 as wire,
    resource_owner::{
        Outcome, Quiescence, validate_prepare, validate_quiescence_observation,
        validate_quiescence_request, validate_response,
    },
};
use prost::Message;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendFlags, recvmsg, send,
    },
};
use std::{
    io,
    io::IoSliceMut,
    mem::MaybeUninit,
    os::fd::{AsFd, AsRawFd, OwnedFd},
    time::{Duration, Instant},
};

/// The live feed of the specific issue. The cleanup responses are just observations.
pub struct OwnerSession {
    fd: OwnedFd,
    prepare: wire::Prepare,
    requests: std::collections::BTreeSet<[u8; 16]>,
    terminal: bool,
    dispatched: bool,
}

#[path = "owner_dispatch.rs"]
mod owner_dispatch;

impl OwnerSession {
    /// Checks one step; after an error or a final answer, repeating is forbidden.
    /// Only Draining allows for a unique new request_idNone of the results of this operation
    /// create QuiescedLease And it doesn't remove the durable fence.
    pub fn inspect_quiescence(
        &mut self,
        request_id: [u8; 16],
        timeout: Duration,
    ) -> io::Result<Quiescence> {
        if self.terminal {
            return Err(invalid());
        }
        if self.requests.len() >= carosign_protocol::resource_owner::QUIESCENCE_REQUEST_LIMIT
            || !self.requests.insert(request_id)
        {
            self.terminal = true;
            return Err(invalid());
        }
        // The outcome of the transfer may be unknown even if there is a local error.
        self.terminal = true;
        let request = wire::InspectQuiescence {
            request_id: request_id.to_vec(),
            boot_id: self.prepare.boot_id.clone(),
            scope: self.prepare.scope.clone(),
        };
        validate_quiescence_request(&self.prepare, &request).map_err(|_| invalid())?;
        let deadline = Instant::now().checked_add(timeout).ok_or_else(invalid)?;
        let encoded = request.encode_to_vec();
        if encoded.is_empty() || encoded.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(invalid());
        }
        wait(&self.fd, PollFlags::OUT, deadline)?;
        if send(
            &self.fd,
            &encoded,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )? != encoded.len()
        {
            return Err(io::ErrorKind::WriteZero.into());
        }
        wait(&self.fd, PollFlags::IN, deadline)?;
        let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
        // Without an ancillary buffer, the kernel closes received `SCM_RIGHTS` descriptors.
        // Reject frames with `MSG_CTRUNC`. The observation channel never carries
        // descriptors.
        let mut control = RecvAncillaryBuffer::default();
        let received = recvmsg(
            &self.fd,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut control,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        )?;
        if received.bytes == 0
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(invalid());
        }
        let response =
            wire::QuiescenceObservation::decode(&bytes[..received.bytes]).map_err(|_| invalid())?;
        if response.encode_to_vec() != bytes[..received.bytes] {
            return Err(invalid());
        }
        let stage = validate_quiescence_observation(&self.prepare, &request, &response, 0)
            .map_err(|_| invalid())?;
        if stage == Quiescence::Draining {
            self.terminal = false;
        }
        Ok(stage)
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "неверный ответ владельца ресурсов",
    )
}

fn check_channel(fd: &OwnedFd) -> io::Result<()> {
    use rustix::net::{SocketType, sockopt::socket_type};
    if socket_type(fd)? != SocketType::SEQPACKET {
        return Err(invalid());
    }
    if !crate::transport::unnamed_unix(fd.as_raw_fd(), false)
        || !crate::transport::unnamed_unix(fd.as_raw_fd(), true)
    {
        return Err(invalid());
    }
    // SAFETY: fd belongs to this call and remains open at fcntl.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
    if flags < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn wait<F: AsFd>(fd: &F, flags: PollFlags, deadline: Instant) -> io::Result<()> {
    let duration = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
    let timeout = Timespec {
        tv_sec: duration.as_secs().try_into().map_err(|_| invalid())?,
        tv_nsec: duration.subsec_nanos().into(),
    };
    let mut fds = [PollFd::new(fd, flags)];
    if poll(&mut fds, Some(&timeout))? == 0 {
        return Err(io::ErrorKind::TimedOut.into());
    }
    if !fds[0].revents().contains(flags) {
        return Err(io::ErrorKind::BrokenPipe.into());
    }
    Ok(())
}

/// Performs exactly one request, and if successful, it passes the channel to the owner of the
/// issuer so that the owner can hold on to copies until the end of the life cycle.
/// The caller checks the policy and origin of the owner and secures the exact scope.Error or
/// lost response does not remove the fence.
pub fn prepare_once(fd: OwnedFd, request: &wire::Prepare, timeout: Duration) -> io::Result<Reply> {
    validate_prepare(request).map_err(|_| invalid())?;
    check_channel(&fd)?;
    let deadline = Instant::now().checked_add(timeout).ok_or_else(invalid)?;
    let request_bytes = request.encode_to_vec();
    if request_bytes.is_empty() || request_bytes.len() > carosign_protocol::limits::FRAME_MAX {
        return Err(invalid());
    }
    wait(&fd, PollFlags::OUT, deadline)?;
    let sent = send(
        &fd,
        &request_bytes,
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )?;
    if sent != request_bytes.len() {
        return Err(io::ErrorKind::WriteZero.into());
    }
    wait(&fd, PollFlags::IN, deadline)?;
    let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let result = recvmsg(
        &fd,
        &mut [IoSliceMut::new(&mut bytes)],
        &mut control,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )?;
    if result.bytes == 0
        || result
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    {
        return Err(invalid());
    }
    let mut descriptors = Vec::new();
    for message in control.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(fds) => descriptors.extend(fds),
            _ => return Err(invalid()),
        }
    }
    let response = wire::Response::decode(&bytes[..result.bytes]).map_err(|_| invalid())?;
    // Prost discards unknown and duplicate fields: only one canonical frame can be taken
    // without ambiguous hidden content.
    if response.encode_to_vec() != bytes[..result.bytes] {
        return Err(invalid());
    }
    match validate_response(request, &response, descriptors.len()).map_err(|_| invalid())? {
        Outcome::Prepared => Ok(Reply::Prepared {
            channel: OwnerSession {
                fd,
                prepare: request.clone(),
                requests: std::collections::BTreeSet::new(),
                terminal: false,
                dispatched: false,
            },
            descriptors,
        }),
        Outcome::Refused => match response.result {
            Some(wire::response::Result::Refused(error)) => Ok(Reply::Refused(error)),
            _ => Err(invalid()),
        },
    }
}

#[path = "owner_session.tests.rs"]
#[cfg(test)]
mod tests;
