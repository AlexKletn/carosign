use super::grant_outcome::GrantOutcome;
use carosign_core::ids::LeaseId;
use carosign_core::resource::ResourceSet;
use carosign_runtime::ports::QuiesceAssessment;

/// That's what the adapter's middleman does.
///
/// ⚠️ The line is narrow: no descriptors, no paths. Descriptors are transmitted by a single
/// binding step, and that's not a runtime thing.
pub trait BrokerClient: Send + Sync {
    /// Requests a set.
    fn acquire(&self, set: &ResourceSet, lease: LeaseId) -> GrantOutcome;
    /// It releases and confirms (or does not confirm) security.
    fn release(&self, lease: LeaseId) -> QuiesceAssessment;
}
