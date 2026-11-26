use super::FileIdentity;

/// Open object metadata for internal verification of the launch path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LaunchMetadata {
    pub identity: FileIdentity,
    pub mode: libc::mode_t,
}
