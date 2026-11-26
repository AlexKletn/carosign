use crate::sys::FileIdentity;
use std::os::fd::OwnedFd;

/// Candidate for further verification; not launch authorization.
pub(super) struct PinnedCandidate {
    pub(super) _root: OwnedFd,
    pub(super) _executable: OwnedFd,
    pub(super) readable_executable: OwnedFd,
    pub(super) identity: FileIdentity,
}
