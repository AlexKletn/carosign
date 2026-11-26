use super::InstallationKey;
use crate::sys::FileIdentity;
use std::path::PathBuf;

/// The constructor is not available outside this module: the manifest text and the client's
/// arguments cannot be such confirmation.
pub(super) struct ApprovedInstallation {
    pub(super) key: InstallationKey,
    pub(super) executable: PathBuf,
    pub(super) root_identity: FileIdentity,
    pub(super) executable_identity: FileIdentity,
}
