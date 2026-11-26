use std::os::fd::OwnedFd;

pub(super) struct ClientEnd {
    pub(super) worker: OwnedFd,
}
