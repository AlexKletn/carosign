/// Direction of the already verified package; verification does not in itself confer rights.
pub(crate) enum CheckedRequest {
    Local {
        request_id: [u8; 16],
    },
    Command {
        request_id: [u8; 16],
        bytes: Vec<u8>,
    },
    Control {
        request_id: [u8; 16],
        bytes: Vec<u8>,
    },
    Bind {
        request_id: [u8; 16],
        bytes: Vec<u8>,
        fds: Vec<std::os::fd::OwnedFd>,
    },
}
