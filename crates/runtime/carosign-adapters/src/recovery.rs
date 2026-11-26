//! Preparation of fences on the verified photo of the magazine.

use carosign_persistence::model::StorageError;
use carosign_persistence::recovery::StartupScan;
use carosign_persistence::store::MetadataStore;
use carosign_runtime::recovery::blocked_scope::{
    RecoveryCoordinator, RecoveryError, RecoveryScope,
};

/// The reason for the failure of the preparation of recovery.
#[derive(Debug)]
pub enum StartupRecoveryError {
    /// The storage image is unavailable or inconsistent.
    Storage(StorageError),
    /// You cannot place all areas in a limited coordinator.
    Coordinator(RecoveryError),
}

/// The coordinator builds the whole or refuses, without giving a partial result.
///
/// The caller must stop the admission and certify the version of the scheme and the
/// completion of the migration, the result does not in itself confirm readiness.
pub fn prepare_startup_recovery_v3(
    store: &dyn MetadataStore,
) -> Result<(StartupScan, RecoveryCoordinator), StartupRecoveryError> {
    let snapshot =
        StartupScan::scan_v3_with_scopes(store).map_err(StartupRecoveryError::Storage)?;
    let mut coordinator = RecoveryCoordinator::new();
    for entry in snapshot.blocked {
        coordinator
            .block(
                RecoveryScope {
                    subject: entry.subject,
                    authenticator: entry.authenticator,
                },
                entry.mutation,
            )
            .map_err(StartupRecoveryError::Coordinator)?;
    }
    Ok((snapshot.scan, coordinator))
}
