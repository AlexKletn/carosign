use super::CleanupPortFailure;
use super::CleanupScope;
use super::QuiescedLease;
use super::TrustedCleanupBroker;
use crate::ports::ResourceOwner;
use carosign_core::ids::LeaseId;
use carosign_core::resource::ResourceSet;

/// Repeated confirmation of the response of a trusted broker.
pub struct Released {
    scope: CleanupScope,
    fence: Option<std::sync::Arc<()>>,
}

impl Released {
    /// Sustainablely removes the fence only after release by a trusted broker and
    /// reconciliation with the withheld issuance of the scheduler.
    /// In case of a fixation error, the confirmation is absorbed: the operational fence
    /// should remain closed, since the outcome of the recording may be unknown.
    pub(in crate::scheduler) fn release_durable_fence(
        self,
        store: &carosign_persistence::redb::backend::InitialFenceStore,
        held: &super::super::Quiescing,
    ) -> Result<Self, carosign_persistence::model::StorageError> {
        use carosign_persistence::model::StorageError;
        use carosign_persistence::redb::backend::{DurableReleasePermit, FenceScope};

        if !self.matches(held.lease, held.owner, &held.set, &held.key) {
            return Err(StorageError::Unavailable);
        }
        let scope = FenceScope {
            lease: held.lease,
            operation: held.owner.operation,
            worker: held.owner.worker,
            authenticator: held.owner.authenticator,
            resources: held.set.clone(),
        };
        // SAFETY: `Released` is obtained only after matching quiescence and release replies from the
        // trusted broker. All fields and the unique key of the fence held by the scheduler
        // were checked above.
        let permit = unsafe { DurableReleasePermit::from_verified_quiescence(scope) };
        store.release_verified(permit)?;
        Ok(self)
    }

    pub(in crate::scheduler) fn matches(
        &self,
        lease: LeaseId,
        owner: ResourceOwner,
        set: &ResourceSet,
        key: &std::sync::Arc<()>,
    ) -> bool {
        self.scope.lease == lease
            && self.scope.owner == owner
            && &self.scope.set == set
            && self
                .fence
                .as_ref()
                .is_some_and(|fence| std::sync::Arc::ptr_eq(fence, key))
    }
}

/// Returns a one-time release after a matching response from a trusted port.
pub async fn confirm_released(
    broker: &dyn TrustedCleanupBroker,
    scope: &CleanupScope,
    proof: QuiescedLease,
) -> Result<Released, CleanupPortFailure> {
    if proof.scope != *scope {
        return Err(CleanupPortFailure::Uncertain);
    }
    let fence = proof.fence.clone();
    if broker.release(scope, proof).await? != *scope {
        return Err(CleanupPortFailure::Uncertain);
    }
    Ok(Released {
        scope: scope.clone(),
        fence,
    })
}

#[cfg(test)]
#[path = "released.tests.rs"]
mod durable_tests;
