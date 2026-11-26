use std::os::fd::OwnedFd;

pub(crate) struct QueuedPacket {
    pub(crate) bytes: Vec<u8>,
    pub(crate) fds: Vec<OwnedFd>,
    pub(crate) is_bind: bool,
    pub(crate) request_id: Option<[u8; 16]>,
}
