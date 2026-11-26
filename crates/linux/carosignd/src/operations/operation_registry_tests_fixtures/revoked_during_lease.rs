use super::*;

pub(super) struct RevokedDuringLease<'a> {
    pub(super) registry: &'a OperationRegistry,
    pub(super) step: std::sync::Mutex<Option<carosign_runtime::operation::actor::Step>>,
}

impl crate::resources::MutationLeaseVerifier for RevokedDuringLease<'_> {
    fn validate<'a>(
        &'a self,
        scope: &'a crate::resources::CleanupScope,
    ) -> carosign_runtime::ports::BoxFuture<'a, Result<(), carosign_runtime::ports::ResourceDenial>>
    {
        let step = self
            .registry
            .revoke_verification(scope.owner().operation)
            .unwrap();
        *self.step.lock().unwrap() = Some(step);
        Box::pin(async { Ok(()) })
    }
}
