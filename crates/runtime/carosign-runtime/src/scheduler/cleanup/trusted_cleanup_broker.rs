use super::CleanupPortFailure;
use super::CleanupScope;
use super::QuiescedLease;
use crate::ports::BoxFuture;

/// The implementation is chosen by the trusted staff; the worker cannot issue
/// confirmations.
pub trait TrustedCleanupBroker: Send + Sync {
    /// The composition confirmed the termination of the sender and the worker on the OS
    /// processes.
    /// The absence of such confirmation leaves the unknown shipment closed.
    fn processes_stopped(&self, _scope: &CleanupScope) -> bool {
        false
    }
    fn quiesce<'a>(
        &'a self,
        scope: &'a CleanupScope,
    ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>>;
    fn release<'a>(
        &'a self,
        scope: &'a CleanupScope,
        proof: QuiescedLease,
    ) -> BoxFuture<'a, Result<CleanupScope, CleanupPortFailure>>;
}
