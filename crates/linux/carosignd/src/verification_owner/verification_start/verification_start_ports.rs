use super::{EffectOwner, OperationRegistry};

pub(crate) struct VerificationStartPorts<'a> {
    pub registry: &'a OperationRegistry,
    pub effects: &'a EffectOwner,
    pub authority: &'a dyn carosign_runtime::ports::PolicyAuthority,
    pub clock: &'a dyn carosign_runtime::ports::TimeSource,
    pub broker: &'a dyn crate::resources::TrustedCleanupBroker,
    pub scheduler: &'a std::sync::Mutex<carosign_runtime::scheduler::Scheduler>,
}
