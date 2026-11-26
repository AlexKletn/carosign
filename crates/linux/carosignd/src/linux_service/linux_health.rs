use super::{CapabilityState, MutationActivity};

pub(super) struct LinuxHealth<'a, S> {
    pub(super) store: &'a S,
    pub(super) policy: &'a crate::linux_policy::LinuxPolicy,
    pub(super) capabilities: &'a std::sync::Mutex<CapabilityState>,
    pub(super) activity: &'a MutationActivity,
}

impl<S: carosign_persistence::store::MetadataStore> crate::health::HealthSource
    for LinuxHealth<'_, S>
{
    fn snapshot(&self) -> Result<crate::health::HealthSnapshot, crate::health::HealthUnavailable> {
        let active = self
            .activity
            .0
            .lock()
            .map_err(|_| crate::health::HealthUnavailable)?;
        if *active != 0 {
            return Err(crate::health::HealthUnavailable);
        }
        let tx = self
            .store
            .begin_read()
            .map_err(|_| crate::health::HealthUnavailable)?;
        let scan = carosign_persistence::mutation_store::scan_prepared_v4_snapshot(&*tx)
            .map_err(|_| crate::health::HealthUnavailable)?;
        let capabilities = self
            .capabilities
            .lock()
            .map_err(|_| crate::health::HealthUnavailable)?;
        let available = crate::linux_dispatch::available_capabilities(
            capabilities.descriptor.as_ref(),
            capabilities.verified_ready,
        );
        Ok(crate::health::HealthSnapshot {
            startup_complete: true,
            listener_published: true,
            policy_ready: self.policy.is_current(),
            available_authenticators: u32::from(available),
            unavailable_authenticators: u32::from(!available),
            scopes_needing_reconciliation: u32::try_from(scan.blocked.len()).unwrap_or(u32::MAX),
        })
    }
}
