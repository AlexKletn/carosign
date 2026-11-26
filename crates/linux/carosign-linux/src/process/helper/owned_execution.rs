use std::ffi::CString;
use std::os::fd::OwnedFd;

/// All FDs that the assistant has the right to hold after the root change.
/// The origin of the files and the policy are checked by the caller.
pub(crate) struct OwnedExecution {
    pub(crate) executable: OwnedFd,
    pub(crate) worker: OwnedFd,
    /// Descriptors 0, 1, 2, forwarded to `/dev/null`.
    pub(crate) standard: [OwnedFd; 3],
    pub(crate) args: Vec<CString>,
    pub(crate) allowed_syscalls: Vec<libc::c_long>,
}
