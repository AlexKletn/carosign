use std::io;
use std::os::fd::OwnedFd;

use carosign_linux::sys::PidFd;

pub struct DiscoveredWorker {
    socket: Option<OwnedFd>,
    process: PidFd,
}

impl DiscoveredWorker {
    pub(super) fn new(socket: OwnedFd, process: PidFd) -> Self {
        Self {
            socket: Some(socket),
            process,
        }
    }

    pub(super) fn socket(&self) -> io::Result<&OwnedFd> {
        self.socket
            .as_ref()
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }

    pub fn take_socket(&mut self) -> Option<OwnedFd> {
        self.socket.take()
    }

    pub fn has_exited(&self) -> io::Result<bool> {
        self.process
            .has_exited()
            .map_err(|error| io::Error::from_raw_os_error(error.errno))
    }

    pub fn wait_exited(&self, timeout: std::time::Duration) -> io::Result<bool> {
        self.process
            .wait_exited(timeout)
            .map_err(|error| io::Error::from_raw_os_error(error.errno))
    }

    pub fn stop(&self) -> io::Result<()> {
        self.process
            .kill()
            .map_err(|error| io::Error::from_raw_os_error(error.errno))
    }
}

impl Drop for DiscoveredWorker {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
