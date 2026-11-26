pub(super) struct FixtureHealth(pub(super) std::sync::atomic::AtomicU32);

impl carosignd::health::HealthSource for FixtureHealth {
    fn snapshot(
        &self,
    ) -> Result<carosignd::health::HealthSnapshot, carosignd::health::HealthUnavailable> {
        Ok(carosignd::health::HealthSnapshot {
            startup_complete: true,
            listener_published: true,
            policy_ready: true,
            available_authenticators: 2 + self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            unavailable_authenticators: 3,
            scopes_needing_reconciliation: 0,
        })
    }
}
