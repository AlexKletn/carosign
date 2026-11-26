use super::{
    BTreeMap, CLEANUP_RESULTS_MAX, CleanupClaim, CleanupOwnerError, CleanupResult, Future, Mutex,
    OperationId, OperationRegistry, PlannedEffect, Poll, Reservation, Slot, TrustedCleanupBroker,
    poll_fn,
};

/// Internal owner; does not confirm hardware security.
#[derive(Default)]
pub struct CleanupOwner {
    pub(super) slots: Mutex<BTreeMap<OperationId, Slot>>,
}

impl CleanupOwner {
    /// Protects the scheduler’s resources until the mediator is waiting.
    pub(crate) async fn execute_scheduled<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
    ) -> Result<(), CleanupOwnerError> {
        self.execute_scheduled_inner(registry, id, effect, scheduler, broker, timeout, None)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn execute_scheduled_at<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), CleanupOwnerError> {
        self.execute_scheduled_inner(
            registry,
            id,
            effect,
            scheduler,
            broker,
            timeout,
            Some(clock),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_scheduled_inner<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
        clock: Option<&dyn carosign_runtime::ports::TimeSource>,
    ) -> Result<(), CleanupOwnerError> {
        {
            let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            if slots.contains_key(&id) {
                return Err(CleanupOwnerError::Duplicate);
            }
            if slots.len() >= CLEANUP_RESULTS_MAX {
                return Err(CleanupOwnerError::Limit);
            }
            slots.insert(id, Slot::Running);
        }
        let mut reservation = Reservation {
            owner: self,
            id,
            finished: false,
        };
        let mut cleanup: std::pin::Pin<Box<dyn Future<Output = _> + '_>> = match clock {
            Some(clock) => Box::pin(
                registry.cleanup_verification_scheduled_at(id, effect, scheduler, broker, clock),
            ),
            None => {
                Box::pin(registry.cleanup_verification_scheduled(id, effect, scheduler, broker))
            }
        };
        let mut timeout = Box::pin(timeout);
        let mut started = false;
        let result = poll_fn(|cx| {
            if !started {
                started = true;
                // The first survey sets the fence before checking the timer.
                if let Poll::Ready(value) = cleanup.as_mut().poll(cx) {
                    return Poll::Ready(Some(value));
                }
                if timeout.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(None);
                }
                return Poll::Pending;
            }
            if timeout.as_mut().poll(cx).is_ready() {
                return Poll::Ready(None);
            }
            match cleanup.as_mut().poll(cx) {
                Poll::Ready(value) => Poll::Ready(Some(value)),
                Poll::Pending => Poll::Pending,
            }
        })
        .await;
        drop(cleanup);
        let result = match result {
            Some(value) => CleanupResult::Scheduled(value),
            None => CleanupResult::Interrupted,
        };
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Slot::Ready(result));
        reservation.finished = true;
        Ok(())
    }
    /// Performs one waiting effect with an external time limiter.
    /// The result is stored until it is explicitly extracted; the interrupted wait does not
    /// prove release. The limiter must complete in time.
    #[cfg(test)]
    pub async fn execute<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
    ) -> Result<(), CleanupOwnerError> {
        {
            let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            if slots.contains_key(&id) {
                return Err(CleanupOwnerError::Duplicate);
            }
            if slots.len() >= CLEANUP_RESULTS_MAX {
                return Err(CleanupOwnerError::Limit);
            }
            slots.insert(id, Slot::Running);
        }
        let mut reservation = Reservation {
            owner: self,
            id,
            finished: false,
        };
        let mut cleanup = Box::pin(registry.cleanup_verification(id, effect, broker));
        let mut timeout = Box::pin(timeout);
        let result = poll_fn(|cx| {
            if timeout.as_mut().poll(cx).is_ready() {
                return Poll::Ready(None);
            }
            match cleanup.as_mut().poll(cx) {
                Poll::Ready(value) => Poll::Ready(Some(value)),
                Poll::Pending => Poll::Pending,
            }
        })
        .await;
        drop(cleanup);
        let result = match result {
            Some(value) => CleanupResult::Completed(value),
            None => CleanupResult::Interrupted,
        };
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Slot::Ready(result));
        reservation.finished = true;
        Ok(())
    }

    /// Once takes the finished result; the working call does not remove.
    pub fn take(&self, id: OperationId) -> Option<CleanupResult> {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(slots.get(&id), Some(Slot::Ready(_))) {
            return None;
        }
        match slots.remove(&id) {
            Some(Slot::Ready(value)) => Some(value),
            _ => None,
        }
    }

    /// If the recipient does not confirm the reception, the result is returned to the box.
    pub(crate) fn claim(&self, id: OperationId) -> Option<CleanupClaim<'_>> {
        let mut slots = self.slots.lock().unwrap_or_else(|error| error.into_inner());
        let slot = slots.get_mut(&id)?;
        if !matches!(slot, Slot::Ready(_)) {
            return None;
        }
        let Slot::Ready(result) = std::mem::replace(slot, Slot::Claimed) else {
            unreachable!()
        };
        Some(CleanupClaim {
            owner: self,
            id,
            result: Some(result),
        })
    }
}
