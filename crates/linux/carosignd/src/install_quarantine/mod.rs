//! Compounding withholding training with quarantine v4.
//!
//! The recording does not open the installation to launch: the re-check here is not
//! serialized with the policy change, and the recovery is not yet connected.

use carosign_linux::discovery::publication_owner::{
    InstallationMetadataDecoder, PublicationFileRole, PublicationRecordFields, QuarantineInputs,
    QuarantinePreparation, RecoveryCheckedFiles, RecoveryExpected, RecoveryFileCandidate,
    ReverifiedContents, ReverifiedProvenance, ReverifyProvenanceInputs, check_recovery_files,
    prepare_quarantine, reverify_contents, reverify_provenance,
};
use carosign_persistence::installation_records::{
    InstallationFile, InstallationFileRole, InstallationState, PackageRecord,
    ProviderInstallationRecord, PublicationFailure, RegisteredProfile,
};
use carosign_persistence::redb::backend::{QuarantineRecoveryStore, RedbStore};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::time::Duration;

impl RecoverySnapshotSource for RedbStore {
    fn read_quarantine(
        &self,
        package: &[u8],
        installation: &[u8],
    ) -> Result<
        carosign_persistence::installation_records::QuarantinedSnapshot,
        carosign_persistence::installation_records::SnapshotRefusal,
    > {
        self.quarantined_installation_snapshot(package, installation)
    }
    fn revalidate_quarantine(
        &self,
        package: &[u8],
        installation: &[u8],
        snapshot: &carosign_persistence::installation_records::QuarantinedSnapshot,
    ) -> Result<(), carosign_persistence::installation_records::SnapshotRefusal> {
        self.revalidate_quarantined_installation(package, installation, snapshot)
    }
}

impl RecoverySnapshotSource for QuarantineRecoveryStore {
    fn read_quarantine(
        &self,
        package: &[u8],
        installation: &[u8],
    ) -> Result<
        carosign_persistence::installation_records::QuarantinedSnapshot,
        carosign_persistence::installation_records::SnapshotRefusal,
    > {
        // The open quarantine owner does not accept other keys.
        let snapshot = self.snapshot()?;
        if snapshot.installation().package_name.as_bytes() != package
            || [
                snapshot.installation().package_id.as_slice(),
                snapshot.installation().profile_id.as_slice(),
            ]
            .concat()
                != installation
        {
            return Err(carosign_persistence::installation_records::SnapshotRefusal::Changed);
        }
        Ok(snapshot)
    }
    fn revalidate_quarantine(
        &self,
        package: &[u8],
        installation: &[u8],
        snapshot: &carosign_persistence::installation_records::QuarantinedSnapshot,
    ) -> Result<(), carosign_persistence::installation_records::SnapshotRefusal> {
        self.read_quarantine(package, installation)?;
        self.revalidate(snapshot)
    }
}

#[path = "install_quarantine.tests.rs"]
#[cfg(test)]
mod tests;

mod quarantine_failure;
pub(crate) use quarantine_failure::QuarantineFailure;
mod reverified_quarantine;
pub(crate) use reverified_quarantine::ReverifiedQuarantine;
mod recovery_snapshot_source;
pub(crate) use recovery_snapshot_source::RecoverySnapshotSource;
mod recovery_inputs;
pub(crate) use recovery_inputs::RecoveryInputs;
mod trusted_recovery_root;
pub(crate) use trusted_recovery_root::TrustedRecoveryRoot;

mod reverify_quarantine;
#[allow(unused_imports)]
pub(crate) use reverify_quarantine::reverify_quarantine;
mod publication;
#[allow(unused_imports)]
pub(crate) use publication::commit;
pub(crate) use publication::prepare_and_commit;
#[cfg(test)]
use publication::records;
mod fingerprint;
use fingerprint::decode_fingerprint;
