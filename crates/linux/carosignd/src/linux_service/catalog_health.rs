use crate::health::{HealthSnapshot, HealthSource, HealthUnavailable};
use crate::provider_catalog::ProviderCatalog;

pub(super) struct CatalogHealth<'a, S> {
    pub(super) store: &'a S,
    pub(super) policy: &'a crate::linux_policy::LinuxPolicy,
    pub(super) catalog: &'a ProviderCatalog,
}

impl<S: carosign_persistence::store::MetadataStore> HealthSource for CatalogHealth<'_, S> {
    fn snapshot(&self) -> Result<HealthSnapshot, HealthUnavailable> {
        let inventory = self.catalog.inventory().map_err(|_| HealthUnavailable)?;
        let tx = self.store.begin_read().map_err(|_| HealthUnavailable)?;
        let scan = carosign_persistence::mutation_store::scan_prepared_v4_snapshot(&*tx)
            .map_err(|_| HealthUnavailable)?;
        let available = inventory.iter().filter(|entry| entry.available).count() as u32;
        Ok(HealthSnapshot {
            startup_complete: true,
            listener_published: true,
            policy_ready: self.policy.is_current(),
            available_authenticators: available,
            unavailable_authenticators: inventory.len() as u32 - available,
            scopes_needing_reconciliation: u32::try_from(scan.blocked.len()).unwrap_or(u32::MAX),
        })
    }
}
