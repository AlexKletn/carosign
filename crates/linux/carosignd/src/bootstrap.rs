//! Restoration before publication: order is more important than speed.
//!
//! ⚠️ Rule 05: The listener is published before the recovery is complete, otherwise the
//! client will come to the service, which does not yet know which areas are closed.
//!
//! ⚠️ Zero ISPs and an empty log are acceptable, but by themselves do not confirm the vault
//! version and migration status.

use core::fmt;

use carosign_core::ids::MutationId;
use carosign_persistence::recovery::{BlockedMutationScope, StartupScan};
use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::store::MetadataStore;

/// Prepare the vault and find out what's ready.
///
/// ⚠️ The damaged base is being re-created: `open` It fails, and the service doesn't go up,
/// which is better than an empty service with lost registrations.
pub fn recover(store_path: &str) -> Result<(RedbStore, Recovered, Readiness), BootstrapError> {
    let store = RedbStore::open(std::path::Path::new(store_path))
        .map_err(|error| BootstrapError::Store(error.to_string()))?;
    recover_opened(store)
}

/// For the main process: checks the circuit and holds the unfinished areas.
pub fn recover_guarded(
    store_path: &str,
) -> Result<(RedbStore, Recovered, Readiness), BootstrapError> {
    recover_guarded_inner(store_path, None)
}

pub fn recover_guarded_with_evidence(
    store_path: &str,
    source: &dyn ArmedRecoverySource,
) -> Result<(RedbStore, Recovered, Readiness), BootstrapError> {
    recover_guarded_inner(store_path, Some(source))
}

fn recover_guarded_inner(
    store_path: &str,
    source: Option<&dyn ArmedRecoverySource>,
) -> Result<(RedbStore, Recovered, Readiness), BootstrapError> {
    use carosign_persistence::mutation_store::{armed_recovery_targets_v4, recover_armed_v4};
    let path = std::path::Path::new(store_path);
    let store = RedbStore::open_current_guarded(path)
        .map_err(|error| BootstrapError::Store(error.to_string()))?;
    if let Some(source) = source {
        let targets = armed_recovery_targets_v4(&store)
            .map_err(|error| BootstrapError::Scan(error.to_string()))?;
        for (mutation, operation) in targets {
            recover_armed_v4(
                &store,
                mutation,
                operation,
                |snapshot| source.verify(snapshot),
                |record| source.confirm_cleanup(record),
            )
            .map_err(|error| BootstrapError::Scan(format!("{error:?}")))?;
        }
    }
    let snapshot = store
        .begin_read()
        .map_err(|error| BootstrapError::Store(error.to_string()))?;
    let scan = carosign_persistence::mutation_store::scan_prepared_v4_snapshot(&*snapshot)
        .map_err(|error| BootstrapError::Scan(error.to_string()))?;
    drop(snapshot);
    let mut blocked = scan.scan.safe_to_close;
    blocked.extend_from_slice(&scan.scan.needing_reconciliation);
    // NoChange does not confirm cleaning: until the owner decides, the area remains fenced in
    // the same way as other uncompleted changes.
    blocked.extend_from_slice(&scan.scan.needs_decision);
    blocked.sort();
    blocked.dedup();
    let recovered = Recovered {
        schema: SchemaStatus::V4,
        needs_decision: blocked.clone(),
        blocked,
        blocked_scopes: scan.blocked,
    };
    // Area consistency and fencing are required before the caller can link the recovery to
    // the rest of the ports.
    let _ = recovered.coordinator()?;
    Ok((store, recovered, Readiness::NotReady))
}

fn recover_opened(store: RedbStore) -> Result<(RedbStore, Recovered, Readiness), BootstrapError> {
    let verified = store
        .has_schema_v3()
        .map_err(|error| BootstrapError::Store(error.to_string()))?;
    let (scan, blocked_scopes) = if verified {
        let result = StartupScan::scan_v3_with_scopes(&store as &dyn MetadataStore)
            .map_err(|error| BootstrapError::Scan(error.to_string()))?;
        (result.scan, result.blocked)
    } else {
        let scan = StartupScan::scan(&store as &dyn MetadataStore)
            .map_err(|error| BootstrapError::Scan(error.to_string()))?;
        (scan, Vec::new())
    };

    let mut blocked = scan.needing_reconciliation.clone();
    // Prepared is not yet closed by the transaction and its blocking remains in the database.
    blocked.extend_from_slice(&scan.safe_to_close);
    blocked.extend_from_slice(&scan.needs_decision);
    blocked.sort();
    blocked.dedup();
    // Neither old nor v3 Prepared are closed here: fixing and cleaning are not yet connected,
    // so the entire unfinished area remains closed.
    let recovered = Recovered {
        schema: if verified {
            SchemaStatus::V3
        } else {
            SchemaStatus::Unverified
        },
        needs_decision: blocked.clone(),
        blocked,
        blocked_scopes,
    };
    if verified {
        let _ = recovered.coordinator()?;
    }
    // The marker and scan are not yet ready for the remaining ports of service.
    let readiness = Readiness::NotReady;
    Ok((store, recovered, readiness))
}

#[path = "bootstrap.tests.rs"]
#[cfg(test)]
mod tests;

mod recovered;
pub use recovered::Recovered;
mod schema_status;
pub use schema_status::SchemaStatus;
mod readiness;
pub use readiness::Readiness;
mod bootstrap_error;
pub use bootstrap_error::BootstrapError;

mod armed_recovery_source;
pub use armed_recovery_source::ArmedRecoverySource;
