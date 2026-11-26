use carosign_core::ids::{PackageId, ProfileId, ProviderId};

use super::PublicationFileRole;

/// Fields derived solely from withheld training: This is a shot for future quarantine
/// publication, not a launch authorization or commit.
pub struct PublicationRecordFields {
    pub package: PackageId,
    pub provider: ProviderId,
    pub profile: ProfileId,
    pub policy_id: [u8; 16],
    pub package_name: String,
    pub version: String,
    pub profile_name: String,
    pub version_dir: String,
    pub device: u64,
    pub inode: u64,
    pub signer_version: u8,
    pub signer_fingerprint: String,
    pub profile_path: String,
    pub executable_path: String,
    pub files: Vec<(String, PublicationFileRole, [u8; 32])>,
    /// Measurements of saved signed archive and detached signature.
    pub provenance_verity: [[u8; 32]; 2],
}
