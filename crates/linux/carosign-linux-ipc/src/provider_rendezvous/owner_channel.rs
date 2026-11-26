use std::{io, os::fd::OwnedFd, time::Duration};

use carosign_linux::sys;

const READY: [u8; 1] = [1];

pub(super) fn send_owner_channel(
    worker: &OwnedFd,
    channel: &OwnedFd,
    timeout: Duration,
) -> io::Result<()> {
    sys::send_fd(worker, channel).map_err(|error| io::Error::from_raw_os_error(error.errno))?;
    let mut ready = [0; READY.len()];
    if sys::receive_seqpacket(worker, &mut ready, timeout)? != READY.len() || ready != READY {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

pub fn receive_owner_channel(worker: &OwnedFd, timeout: Duration) -> io::Result<OwnedFd> {
    sys::require_unix_seqpacket(worker)?;
    let peer =
        sys::peer_credentials(worker).map_err(|error| io::Error::from_raw_os_error(error.errno))?;
    if peer.uid != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let channel = sys::receive_fd_until(worker, timeout)?;
    sys::require_unix_seqpacket(&channel)?;
    sys::send_seqpacket(worker, &READY)?;
    Ok(channel)
}
