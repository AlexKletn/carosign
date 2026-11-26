use super::ExecDisposition;
use std::os::fd::RawFd;

/// Clearly authorized assistant descriptor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AllowedFd {
    pub(crate) fd: RawFd,
    pub(crate) disposition: ExecDisposition,
}
