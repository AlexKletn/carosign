use super::{PackageRecord, ProviderInstallationRecord};

/// Checks quarantine and both records in the same image to prepare for recovery.
/// The return entry does not certify origin or permit launch; quarantine itself never alters
/// this method.
#[derive(Debug, PartialEq)]
pub struct QuarantinedSnapshot {
    pub(in super::super) package: PackageRecord,
    pub(in super::super) installation: ProviderInstallationRecord,
    pub(in super::super) provenance: Option<[u8; 64]>,
}

impl QuarantinedSnapshot {
    /// Unverified entry: does not in itself confirm origin.
    pub fn installation(&self) -> &ProviderInstallationRecord {
        &self.installation
    }

    /// Measurements of stored archives and signatures, if recorded.
    pub fn provenance_verity(&self) -> Option<[[u8; 32]; 2]> {
        let bytes = self.provenance?;
        Some([bytes[..32].try_into().ok()?, bytes[32..].try_into().ok()?])
    }
}
