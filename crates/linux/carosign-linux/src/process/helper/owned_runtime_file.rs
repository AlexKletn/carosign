use std::os::fd::OwnedFd;

/// Full ownership of the files is transferred to a separate assistant before launch.
pub(crate) struct OwnedRuntimeFile {
    pub(crate) path: String,
    pub(crate) source: OwnedFd,
}
