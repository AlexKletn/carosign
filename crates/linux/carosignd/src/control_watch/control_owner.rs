use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use carosign_core::ids::OperationId;
use carosign_runtime::ports::TimeSource;

use crate::operations::{ObservationFailure, OperationRegistry};

use super::{
    ControlClaim, RevocationSender, StartError, WatchHandle, WatchResult,
    revocation_signal::RevocationSignal, slot::Slot,
};

const WATCH_MAX: usize = 64;
const CHECK_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Default)]
pub(crate) struct ControlOwner {
    pub(super) slots: Mutex<BTreeMap<OperationId, Slot>>,
}

impl ControlOwner {
    pub(crate) fn retire_completed(&self, registry: &OperationRegistry) {
        let mut slots = self.slots.lock().unwrap_or_else(|error| error.into_inner());
        slots.retain(|operation, slot| {
            !matches!(slot, Slot::Ready(_)) || !registry.is_completed(*operation)
        });
    }

    /// The signal is already stored and is awaiting processing by the owner of the operation.
    pub(crate) fn ready(&self, operation: OperationId) -> bool {
        self.slots
            .lock()
            .is_ok_and(|slots| matches!(slots.get(&operation), Some(Slot::Ready(_))))
    }
    /// One observer with a limited lifespan is launched.
    /// Its source and watch choose a trusted line-up, not a client.
    pub fn start(
        self: &Arc<Self>,
        registry: Arc<OperationRegistry>,
        operation: OperationId,
        clock: Arc<dyn TimeSource>,
        lifetime: Duration,
    ) -> Result<(WatchHandle, RevocationSender), StartError> {
        self.start_observing(registry, operation, clock, lifetime, None)
    }

    /// Observing a change does not confirm the origin of the policy; the caller is obliged to
    /// separately check the snapshot and trusted path chain.
    pub(crate) fn start_observing(
        self: &Arc<Self>,
        registry: Arc<OperationRegistry>,
        operation: OperationId,
        clock: Arc<dyn TimeSource>,
        lifetime: Duration,
        mut policy: Option<carosign_linux::policy::PolicyChangeWatch>,
    ) -> Result<(WatchHandle, RevocationSender), StartError> {
        let end = Instant::now()
            .checked_add(lifetime)
            .ok_or(StartError::InvalidDuration)?;
        if lifetime.is_zero() {
            return Err(StartError::InvalidDuration);
        }
        {
            let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            if slots.contains_key(&operation) {
                return Err(StartError::Duplicate);
            }
            if slots.len() >= WATCH_MAX {
                return Err(StartError::Limit);
            }
            slots.insert(operation, Slot::Running);
        }
        let (sender, source) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let owner = Arc::clone(self);
        let thread = match std::thread::Builder::new()
            .name("carosign-control-watch".into())
            .spawn(move || {
                let mut cancellation_ack = None;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    loop {
                        // Any loss of observation or change prohibits success.
                        if let Some(watch) = policy.as_mut()
                            && !matches!(
                                watch.check(),
                                Ok(carosign_linux::policy::PolicyWatchState::Unchanged)
                            )
                        {
                            eprintln!("carosignd: отзыв проверки: изменение или потеря наблюдения политики");
                            break registry.revoke_verification(operation).map(Some);
                        }
                        // If supervision is stopped or lost, further success is prohibited.
                        if stopped.load(Ordering::Acquire) || Instant::now() >= end {
                            eprintln!("carosignd: отзыв проверки: останов наблюдения или предел наблюдателя");
                            break registry.revoke_verification(operation).map(Some);
                        }
                        match source.recv_timeout(
                            CHECK_INTERVAL.min(end.saturating_duration_since(Instant::now())),
                        ) {
                            Ok(RevocationSignal::ClientCancel(ack)) => {
                                eprintln!("carosignd: отзыв проверки: отмена клиента");
                                cancellation_ack = Some(ack);
                                break registry.cancel_verification(operation).map(Some);
                            }
                            Ok(RevocationSignal::Revoked) => {
                                eprintln!("carosignd: отзыв проверки: сигнал отзыва");
                                break registry.revoke_verification(operation).map(Some);
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                eprintln!("carosignd: отзыв проверки: источник сигнала закрыт");
                                break registry.revoke_verification(operation).map(Some);
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                match registry.check_verification_deadline(operation, clock.now()) {
                                    Ok(None) => continue,
                                    other => {
                                        eprintln!("carosignd: завершение наблюдения проверки: срок или ошибка состояния");
                                        break other;
                                    }
                                }
                            }
                        }
                    }
                }));
                // A failure of the observer means a loss of supervision, so he too puts out
                // the operation; a panic does not turn into a successful outcome.
                let result = match result {
                    Ok(result) => result,
                    Err(_) => registry.revoke_verification(operation).map(Some),
                };
                let result = match result {
                    Ok(Some(step)) => WatchResult::Step(step),
                    Err(ObservationFailure::Busy) => WatchResult::Deferred,
                    Err(error) => WatchResult::Failed(error),
                    Ok(None) => WatchResult::Failed(ObservationFailure::ContextMismatch),
                };
                let cancellation_saved =
                    matches!(result, WatchResult::Step(_) | WatchResult::Deferred);
                owner
                    .slots
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(operation, Slot::Ready(result));
                if let Some(ack) = cancellation_ack {
                    let _ = ack.send(cancellation_saved);
                }
            }) {
            Ok(thread) => thread,
            Err(_) => {
                self.slots
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&operation);
                return Err(StartError::Spawn);
            }
        };
        Ok((
            WatchHandle {
                stop,
                thread: Some(thread),
            },
            RevocationSender(sender),
        ))
    }

    /// The result remains in the box when the holder is lost; confirmation is only
    /// permissible after the effects have been transmitted to the execution owner.
    pub fn claim(&self, operation: OperationId) -> Option<ControlClaim<'_>> {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let slot = slots.get_mut(&operation)?;
        if !matches!(slot, Slot::Ready(_)) {
            return None;
        }
        let Slot::Ready(result) = std::mem::replace(slot, Slot::Claimed) else {
            unreachable!()
        };
        Some(ControlClaim {
            owner: self,
            operation,
            result: Some(result),
        })
    }
}

#[cfg(test)]
#[path = "control_owner.tests.rs"]
mod tests;
