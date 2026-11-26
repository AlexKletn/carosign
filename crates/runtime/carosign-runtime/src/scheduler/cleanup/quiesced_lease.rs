use super::CleanupPortFailure;
use super::CleanupScope;
use super::TrustedCleanupBroker;

/// One-time confirmation of peace; does not confirm release.
pub struct QuiescedLease {
    pub(super) scope: CleanupScope,
    pub(super) fence: Option<std::sync::Arc<()>>,
}

impl QuiescedLease {
    /// The area where peace has been tested.
    pub fn scope(&self) -> &CleanupScope {
        &self.scope
    }
}

/// Returns the opportunity only after the matching response of the trusted port.
pub async fn confirm_quiesced(
    broker: &dyn TrustedCleanupBroker,
    scope: &CleanupScope,
) -> Result<QuiescedLease, CleanupPortFailure> {
    if broker.quiesce(scope).await? != *scope {
        return Err(CleanupPortFailure::Uncertain);
    }
    Ok(QuiescedLease {
        scope: scope.clone(),
        fence: None,
    })
}

/// It certifies rest for a specific non-repeatable fence.
pub async fn confirm_quiesced_for(
    broker: &dyn TrustedCleanupBroker,
    scope: &CleanupScope,
    held: &super::super::Quiescing,
) -> Result<QuiescedLease, CleanupPortFailure> {
    if held.lease != scope.lease || held.owner != scope.owner || held.set != scope.set {
        return Err(CleanupPortFailure::Uncertain);
    }
    if broker.quiesce(scope).await? != *scope {
        return Err(CleanupPortFailure::Uncertain);
    }
    Ok(QuiescedLease {
        scope: scope.clone(),
        fence: Some(held.key.clone()),
    })
}
