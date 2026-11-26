use super::{
    CleanupFailure, CleanupStage, ObservationFailure, OperationRegistry, OwnedOperation,
    OwnerDeliveryFailure, TerminalRecordFailure, TerminalRecordStage,
};
use carosign_core::ids::OperationId;

/// Returns the actor to a reserved location at any call exit.
pub(crate) struct OperationGuard<'a> {
    pub(super) registry: &'a OperationRegistry,
    pub(super) id: OperationId,
    pub(super) operation: Option<OwnedOperation>,
}

impl OperationGuard<'_> {
    pub fn operation_mut(&mut self) -> &mut OwnedOperation {
        self.operation
            .as_mut()
            .expect("актор принадлежит владельцу")
    }

    pub(crate) fn delivery_invalidated(&self) -> bool {
        self.registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&self.id)
            .is_none_or(|entry| entry.delivery_invalidated)
    }

    pub(super) fn publish_provider_progress(
        &mut self,
        progress: carosign_runtime::operation::events::Progress,
    ) -> Result<Option<carosign_runtime::operation::events::Event>, ObservationFailure> {
        use carosign_engine::OperationPhase;
        use carosign_runtime::operation::events::EventKind;

        let entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(&self.id)
            .ok_or(ObservationFailure::UnknownOperation)?;
        let owned = self.operation.as_mut().ok_or(ObservationFailure::Busy)?;
        let state = &owned.actor.state().common;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() || state.cancel_requested
        {
            return Ok(None);
        }
        if !matches!(
            state.phase,
            OperationPhase::Running | OperationPhase::InFlight
        ) {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut hub = owned
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.phase == OperationPhase::Running
            && hub.stream().last_sequence()
                >= (carosign_runtime::operation::events::HISTORY_MAX as u64 - 2)
        {
            return Ok(None);
        }
        hub.publish(state.phase, EventKind::Progress(progress))
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        Ok(hub.stream().latest().cloned())
    }

    /// Publication and verification of the review constitute one synchronous operation.
    pub(super) fn publish_owner_terminal(&mut self) -> Result<u64, OwnerDeliveryFailure> {
        let entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(&self.id)
            .ok_or(OwnerDeliveryFailure::Operation(
                ObservationFailure::UnknownOperation,
            ))?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(OwnerDeliveryFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Revoked,
            ));
        }
        let owned = self
            .operation
            .as_mut()
            .expect("актор принадлежит владельцу");
        let mut hub = owned
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let disconnected = hub
            .publish(
                carosign_engine::OperationPhase::Terminal,
                carosign_runtime::operation::events::EventKind::Terminal,
            )
            .map_err(OwnerDeliveryFailure::Events)?;
        let sequence = hub.stream().last_sequence();
        owned.terminal_disconnects.extend(disconnected);
        owned.delivery_reserved = true;
        Ok(sequence)
    }

    /// Fixing and setting the receipt of the atomic control.
    pub(super) fn record_verification_terminal(
        &mut self,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<carosign_runtime::operation::actor::Step, TerminalRecordFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, OperationPhase};
        use carosign_runtime::OperationMsg;
        use carosign_runtime::operation::actor::Step;
        let mut entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&self.id)
            .ok_or(TerminalRecordFailure::OperationUnavailable)?;
        let operation = self
            .operation
            .as_mut()
            .ok_or(TerminalRecordFailure::OperationUnavailable)?;
        for message in entry.pending_control.drain(..) {
            let _ = operation.actor.post(message);
        }
        let already_sent = operation.actor.state().common.terminal
            == Some(carosign_engine::TerminalOutcome::FactorAccepted)
            && operation
                .actor
                .state()
                .body
                .verification()
                .is_some_and(|verification| {
                    verification.delivery
                        == carosign_engine::transition::VerificationDelivery::Sent
                });
        let negative = matches!(
            operation.actor.state().common.terminal,
            Some(
                carosign_engine::TerminalOutcome::Cancelled
                    | carosign_engine::TerminalOutcome::Unavailable
                    | carosign_engine::TerminalOutcome::Failed
                    | carosign_engine::TerminalOutcome::NoSample
                    | carosign_engine::TerminalOutcome::NotMatched
            )
        );
        // The recall prohibits delivery of success, but does not in itself prevent locally
        // recording a negative outcome after cleaning.
        if (entry.delivery_invalidated && !already_sent && !negative) || operation.actor.has_work()
        {
            return Err(TerminalRecordFailure::AwaitingActor);
        }
        operation
            .actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::VerificationTerminalRecorded,
            }))
            .map_err(|_| TerminalRecordFailure::MailboxFull)?;
        let step = operation.actor.run_once();
        let Step::Advanced { state, .. } = &step else {
            return Err(TerminalRecordFailure::ContextMismatch);
        };
        let outcome = state
            .common
            .terminal
            .ok_or(TerminalRecordFailure::ContextMismatch)?;
        if state.common.phase != OperationPhase::Terminal {
            return Err(TerminalRecordFailure::ContextMismatch);
        }
        operation.terminal_record_stage = TerminalRecordStage::Recorded { effect, outcome };
        Ok(step)
    }

    /// First, it transfers the deferred control, then puts a receipt.
    /// Under the lock, only short transitions of the actor are performed, without
    /// expectations.
    pub(super) fn post_cleanup_receipt(
        &mut self,
        effect: carosign_engine::PlannedEffect,
        receipt: carosign_engine::EffectReceipt,
    ) -> Result<carosign_runtime::operation::actor::Step, CleanupFailure> {
        let mut entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&self.id)
            .ok_or(CleanupFailure::OperationUnavailable)?;
        let operation = self
            .operation
            .as_mut()
            .ok_or(CleanupFailure::OperationUnavailable)?;
        for message in entry.pending_control.drain(..) {
            let _ = operation.actor.post(message);
        }
        operation
            .actor
            .post(carosign_runtime::OperationMsg::Domain(
                carosign_engine::DomainInput::Receipt { effect, receipt },
            ))
            .map_err(|_| CleanupFailure::MailboxFull)?;
        Ok(operation.actor.run_once())
    }
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        let mut entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut terminal = false;
        if let Some(entry) = entries.get_mut(&self.id) {
            if let Some(operation) = self.operation.as_mut() {
                if matches!(
                    operation.cleanup_stage,
                    CleanupStage::Quiescing | CleanupStage::Releasing
                ) {
                    operation.cleanup_stage = CleanupStage::Uncertain;
                }
                for message in entry.pending_control.drain(..) {
                    // Overflow of control is fixed inside the actor.
                    let _ = operation.actor.post(message);
                }
                let state = operation.actor.state();
                // Phase one is not enough: a mutation may still have a record of the result,
                // and resource harvesting may have remained questionable.
                // Until their full contract is connected, the place is held.
                terminal = matches!(state.body, carosign_engine::OperationBody::Verification(_))
                    && state.common.phase == carosign_engine::OperationPhase::Terminal
                    && !operation.actor.has_work()
                    && !operation.actor.has_control_work()
                    && operation.cleanup_stage == CleanupStage::Released
                    && operation.lease.is_none()
                    && state.common.lease.is_none()
                    && state.common.pending_effects.is_empty()
                    && matches!(operation.terminal_record_stage,
                        TerminalRecordStage::Recorded { outcome, .. }
                            if Some(outcome) == state.common.terminal);
            }
            entry.terminal |= terminal;
            entry.operation = self.operation.take();
        }
        if terminal {
            for usage in entries.connections.values_mut() {
                usage.operations.remove(&self.id);
            }
        }
    }
}
