use std::os::fd::OwnedFd;

pub(super) struct Entry {
    pub(super) name: String,
    pub(super) source: OwnedFd,
}
