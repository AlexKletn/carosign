use super::{
    QuarantineFailure, RecoveryCheckedFiles, RecoverySnapshotSource, ReverifiedContents,
    ReverifiedProvenance, TrustedRecoveryRoot,
};

/// Signature confirmation is associated with a specific snapshot, but does not open a launch.
pub(crate) struct ReverifiedQuarantine {
    pub(super) root: TrustedRecoveryRoot,
    pub(super) snapshot: carosign_persistence::installation_records::QuarantinedSnapshot,
    pub(super) provenance: ReverifiedProvenance,
    pub(super) files: RecoveryCheckedFiles,
    pub(super) contents: ReverifiedContents,
    pub(super) package_key: Vec<u8>,
    pub(super) installation_key: Vec<u8>,
    pub(super) invalidated: bool,
}

impl ReverifiedQuarantine {
    pub(crate) fn revalidate(
        &mut self,
        store: &impl RecoverySnapshotSource,
    ) -> Result<(), QuarantineFailure> {
        if self.invalidated {
            return Err(QuarantineFailure::Preparation);
        }
        // After any refusal, the old confirmation can no longer be revived, even if the
        // source is later restored to its former state.
        self.invalidated = true;
        self.revalidate_sources(store)?;
        self.invalidated = false;
        Ok(())
    }

    fn revalidate_sources(
        &mut self,
        store: &impl RecoverySnapshotSource,
    ) -> Result<(), QuarantineFailure> {
        self.root.revalidate()?;
        self.contents
            .revalidate()
            .map_err(|_| QuarantineFailure::Preparation)?;
        self.root.revalidate()?;
        self.provenance
            .revalidate()
            .map_err(|_| QuarantineFailure::Preparation)?;
        self.files
            .revalidate()
            .map_err(|_| QuarantineFailure::Preparation)?;
        store
            .revalidate_quarantine(&self.package_key, &self.installation_key, &self.snapshot)
            .map_err(|_| QuarantineFailure::Preparation)?;
        self.files
            .revalidate()
            .map_err(|_| QuarantineFailure::Preparation)?;
        self.contents
            .revalidate()
            .map_err(|_| QuarantineFailure::Preparation)?;
        self.provenance
            .revalidate()
            .map_err(|_| QuarantineFailure::Preparation)?;
        // The last check covers the time to check files and policies: the retained FD remains
        // the same even after the directory name is changed.
        self.root.revalidate()
    }
}
