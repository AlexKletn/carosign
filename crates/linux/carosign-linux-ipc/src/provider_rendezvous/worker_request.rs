use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::{Duration, Instant};

use carosign_core::device::AuthenticatorRef;
use carosign_core::ids::WorkerInstanceId;
use carosign_linux::sys::{self, PidFd};
use carosign_protocol::carosign::provider_rendezvous::v1 as wire;
use prost::Message;

use super::{DiscoveredWorker, worker_listener::WorkerListener};

pub struct WorkerRequest {
    pub authenticator: AuthenticatorRef,
    pub worker_id: WorkerInstanceId,
    pub uid: u32,
    pub gid: u32,
    pub declaration_only: bool,
}

impl WorkerRequest {
    pub fn open(
        &self,
        control: &OwnedFd,
        directory: &Path,
        timeout: Duration,
    ) -> io::Result<DiscoveredWorker> {
        self.open_bootstrap(control, directory, timeout, None)
    }

    pub fn open_with_owner_channel(
        &self,
        control: &OwnedFd,
        directory: &Path,
        timeout: Duration,
        owner_channel: &OwnedFd,
    ) -> io::Result<DiscoveredWorker> {
        if self.declaration_only {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        sys::require_unix_seqpacket(owner_channel)?;
        self.open_bootstrap(control, directory, timeout, Some(owner_channel))
    }

    fn open_bootstrap(
        &self,
        control: &OwnedFd,
        directory: &Path,
        timeout: Duration,
        owner_channel: Option<&OwnedFd>,
    ) -> io::Result<DiscoveredWorker> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(io::ErrorKind::InvalidInput)?;
        sys::require_unix_seqpacket(control)?;
        let daemon = sys::peer_credentials(control).map_err(sys_error)?;
        if self.uid == 0 || self.gid == 0 || (daemon.uid, daemon.gid) != (self.uid, self.gid) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let request_id = sys::random_16().map_err(sys_error)?;
        if request_id == [0; 16] {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut nonce = sys::random_16().map_err(sys_error)?.to_vec();
        nonce.extend_from_slice(&sys::random_16().map_err(sys_error)?);
        let listener = WorkerListener::bind(directory, &request_id)?;
        let request = wire::Request {
            version: 1,
            request_id: request_id.to_vec(),
            action: Some(wire::request::Action::OpenSession(wire::OpenSession {
                authenticator_id: self.authenticator.id.as_bytes().to_vec(),
                device_epoch: self.authenticator.device_epoch.value(),
                worker_instance_id: self.worker_id.as_bytes().to_vec(),
                worker_socket: listener
                    .path
                    .to_str()
                    .ok_or(io::ErrorKind::InvalidInput)?
                    .to_owned(),
                nonce: nonce.clone(),
                declaration_only: self.declaration_only,
                owner_channel_required: owner_channel.is_some(),
            })),
        };
        let mut buffer = vec![0; carosign_protocol::limits::FRAME_MAX];
        let size = sys::exchange_seqpacket(
            control,
            &request.encode_to_vec(),
            &mut buffer,
            remaining(deadline)?,
        )?;
        let packet = &buffer[..size];
        let response = wire::Response::decode(packet).map_err(|_| io::ErrorKind::InvalidData)?;
        if response.version != 1
            || response.request_id != request_id
            || !matches!(
                response.result,
                Some(wire::response::Result::SessionOpening(_))
            )
            || response.encode_to_vec() != packet
        {
            return Err(io::ErrorKind::InvalidData.into());
        }

        let (socket, peer) = sys::accept_seqpacket_until(&listener.fd, remaining(deadline)?)?;
        if (peer.uid, peer.gid) != (self.uid, self.gid) || peer.pid == daemon.pid {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let process = PidFd::from_socket_peer(&socket).map_err(sys_error)?;
        let size = sys::receive_seqpacket(&socket, &mut buffer, remaining(deadline)?)?;
        let expected = wire::WorkerProof {
            version: 1,
            request_id: request_id.to_vec(),
            authenticator_id: self.authenticator.id.as_bytes().to_vec(),
            device_epoch: self.authenticator.device_epoch.value(),
            worker_instance_id: self.worker_id.as_bytes().to_vec(),
            nonce,
        };
        if expected.encode_to_vec() != buffer[..size] {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let worker = DiscoveredWorker::new(socket, process);
        if worker.has_exited()? {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if let Some(owner_channel) = owner_channel {
            super::owner_channel::send_owner_channel(
                worker.socket()?,
                owner_channel,
                remaining(deadline)?,
            )?;
            if worker.has_exited()? {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
        }
        Ok(worker)
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or_else(|| io::ErrorKind::TimedOut.into())
}

fn sys_error(error: sys::SysError) -> io::Error {
    io::Error::from_raw_os_error(error.errno)
}
