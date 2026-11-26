use std::collections::BTreeMap;

use crate::sys::FileIdentity;

use super::{ApprovedFile, InstallationKey};

/// All values come from a system source, not a manifesto.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ApprovedInstallation {
    pub(super) key: InstallationKey,
    pub(super) root: FileIdentity,
    pub(super) package: String,
    pub(super) version: String,
    pub(super) profile: String,
    pub(super) version_dir: String,
    pub(super) signer_key_version: u32,
    pub(super) signer_fingerprint: Vec<u8>,
    pub(super) key_policy_id: [u8; 16],
    pub(super) revision: u64,
    pub(super) policy_path: String,
    pub(super) manifest_path: String,
    pub(super) profile_path: String,
    pub(super) executable_path: String,
    /// The role and measurement are approved for each file by the system source.
    pub(super) files: BTreeMap<String, ApprovedFile>,
}
