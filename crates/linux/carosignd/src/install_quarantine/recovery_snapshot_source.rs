/// The source of the image does not give authority to launch and lift quarantine.
pub(crate) trait RecoverySnapshotSource {
    fn read_quarantine(
        &self,
        package: &[u8],
        installation: &[u8],
    ) -> Result<
        carosign_persistence::installation_records::QuarantinedSnapshot,
        carosign_persistence::installation_records::SnapshotRefusal,
    >;
    fn revalidate_quarantine(
        &self,
        package: &[u8],
        installation: &[u8],
        snapshot: &carosign_persistence::installation_records::QuarantinedSnapshot,
    ) -> Result<(), carosign_persistence::installation_records::SnapshotRefusal>;
}
