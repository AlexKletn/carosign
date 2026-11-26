/// Agreed snapshot from the owners of the launch, inventory and recovery.
pub struct HealthSnapshot {
    /// Launch completed under a contract of service.
    pub startup_complete: bool,
    /// Listener posted.
    pub listener_published: bool,
    /// Safe policy established.
    pub policy_ready: bool,
    /// The number of authenticators available.
    pub available_authenticators: u32,
    /// Number of unavailable authenticators.
    pub unavailable_authenticators: u32,
    /// The number of areas requiring reconciliation.
    pub scopes_needing_reconciliation: u32,
}

impl HealthSnapshot {
    pub(crate) fn into_wire(self) -> carosign_protocol::carosign::client::v1::Health {
        carosign_protocol::carosign::client::v1::Health {
            service_ready: self.startup_complete
                && self.listener_published
                && self.policy_ready
                && self.scopes_needing_reconciliation == 0,
            available_authenticators: self.available_authenticators,
            unavailable_authenticators: self.unavailable_authenticators,
            scopes_needing_reconciliation: self.scopes_needing_reconciliation,
        }
    }
}
