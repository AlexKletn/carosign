use super::*;
use std::time::Duration;

pub(super) struct UncertainCleanup<'a> {
    pub(super) control: &'a crate::control_watch::ControlOwner,
    pub(super) operation: OperationId,
    pub(super) sender: Option<&'a crate::control_watch::RevocationSender>,
    pub(super) clock: Option<&'a carosign_adapters::clock::StepClock>,
}

impl crate::resources::TrustedCleanupBroker for UncertainCleanup<'_> {
    fn quiesce<'a>(
        &'a self,
        _: &'a crate::resources::CleanupScope,
    ) -> BoxFuture<'a, Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>>
    {
        if let Some(sender) = self.sender {
            sender.revoke().unwrap();
        }
        if let Some(clock) = self.clock {
            clock.advance(61_000_000_000);
        }
        if self.sender.is_some() || self.clock.is_some() {
            let end = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(claim) = self.control.claim(self.operation) {
                    assert!(matches!(
                        claim.result(),
                        crate::control_watch::WatchResult::Deferred
                    ));
                    // Without confirmation, the result is returned to the box.
                    drop(claim);
                    break;
                }
                assert!(
                    std::time::Instant::now() < end,
                    "наблюдатель не обработал отзыв"
                );
                std::thread::yield_now();
            }
        }
        Box::pin(async { Err(crate::resources::CleanupPortFailure::Uncertain) })
    }
    fn release<'a>(
        &'a self,
        _: &'a crate::resources::CleanupScope,
        _: crate::resources::QuiescedLease,
    ) -> BoxFuture<'a, Result<crate::resources::CleanupScope, crate::resources::CleanupPortFailure>>
    {
        panic!("неподтверждённый покой не освобождает выдачу")
    }
}
