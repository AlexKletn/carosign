use super::*;

/// All identifiers are assigned by the system prior to recording, not by packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InstallationRecord {
    pub(crate) package_id: [u8; 16],
    pub(crate) provider_id: [u8; 16],
    pub(crate) profile_id: [u8; 16],
    pub(crate) root_device: u64,
    pub(crate) root_inode: u64,
    pub(crate) fingerprint: String,
    pub(crate) revision: u64,
    pub(crate) revoked: bool,
    pub(crate) files: BTreeMap<String, [u8; 32]>,
}
