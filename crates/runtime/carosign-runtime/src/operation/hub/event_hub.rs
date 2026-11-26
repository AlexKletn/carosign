use std::collections::BTreeMap;

use carosign_core::ids::OperationId;
use carosign_engine::OperationPhase;

use crate::operation::events::{Event, EventError, EventKind, EventStream};
use crate::operation::observer::{OBSERVERS_HARD_MAX, OBSERVERS_MAX, ObserverQueue};

use super::reservation_stage::ReservationStage;
use super::reserved_terminal::ReservedTerminal;
use super::{
    AttachError, ObservedStep, ReservationError, ReservedSendOutcome, TerminalPublishError,
    TerminalReservation,
};

/// One transaction flow; the hub does not wait for the client to work.
#[derive(Debug)]
pub struct EventHub {
    operation: OperationId,
    identity: std::sync::Arc<()>,
    stream: EventStream,
    observers: BTreeMap<u32, (u64, ObserverQueue)>,
    next_generation: u64,
    terminal_published: bool,
    reservation: Option<ReservedTerminal>,
}

impl EventHub {
    /// Local flow check; does not allow removal of the operation.
    pub fn is_drained_terminal(&self) -> bool {
        self.terminal_published && self.observers.is_empty() && self.reservation.is_none()
    }

    /// Creates an empty stream.
    pub fn new(operation: OperationId) -> Self {
        Self {
            operation,
            identity: std::sync::Arc::new(()),
            stream: EventStream::new(operation),
            observers: BTreeMap::new(),
            next_generation: 0,
            terminal_published: false,
            reservation: None,
        }
    }

    /// Connects the observer after repeating to the current number.
    ///
    /// A fresh authorization is required before the call; the number itself is not a right.
    pub fn attach(&mut self, id: u32, after_sequence: u64) -> Result<(), AttachError> {
        self.attach_with_limit(id, after_sequence, OBSERVERS_MAX)
    }

    pub fn attach_with_limit(
        &mut self,
        id: u32,
        after_sequence: u64,
        limit: usize,
    ) -> Result<(), AttachError> {
        if self.observers.contains_key(&id) {
            return Err(AttachError::Duplicate);
        }
        if self.observers.len() >= limit.min(OBSERVERS_HARD_MAX) {
            return Err(AttachError::Limit);
        }
        if after_sequence != self.stream.last_sequence() {
            return Err(AttachError::CatchupRequired);
        }
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(AttachError::GenerationExhausted)?;
        self.next_generation = generation;
        self.observers
            .insert(id, (generation, ObserverQueue::new(after_sequence)));
        Ok(())
    }

    /// Remove the subscription without changing the status of the operation.
    pub fn detach(&mut self, id: u32) -> bool {
        self.observers.remove(&id).is_some()
    }

    /// The operation to which the flow relates.
    pub fn operation(&self) -> OperationId {
        self.operation
    }

    pub fn generation(&self, id: u32) -> Option<u64> {
        self.observers.get(&id).map(|(generation, _)| *generation)
    }

    pub(crate) fn identity(&self) -> std::sync::Arc<()> {
        std::sync::Arc::clone(&self.identity)
    }

    pub(crate) fn matches_identity(&self, identity: &std::sync::Arc<()>) -> bool {
        std::sync::Arc::ptr_eq(&self.identity, identity)
    }

    pub(crate) fn peek(&self, id: u32) -> Option<&Event> {
        self.observers.get(&id).and_then(|(_, queue)| queue.peek())
    }

    /// It saves the event and returns the numbers of the connections that need to be closed.
    ///
    /// The result marker does not confirm the record; before issuing a terminal change, the
    /// caller must read the stable result and authorize it.
    pub fn publish(
        &mut self,
        phase: OperationPhase,
        kind: EventKind,
    ) -> Result<Vec<u32>, EventError> {
        if self.reservation.is_some() {
            return Err(EventError::Reserved);
        }
        if self.terminal_published {
            return Err(EventError::AlreadyTerminal);
        }
        let sequence = self.stream.publish(phase, kind.clone())?;
        self.terminal_published = kind == EventKind::Terminal;
        let event = Event {
            sequence,
            phase,
            kind,
        };
        let mut disconnected = Vec::new();
        self.observers.retain(|id, (_, queue)| {
            if queue.push(event.clone()).is_err() {
                disconnected.push(*id);
                false
            } else {
                true
            }
        });
        Ok(disconnected)
    }

    /// Reserves the next number for pending positive delivery.
    /// Evaluation, authority, recipient and term are verified separately.
    pub fn reserve_positive_terminal(
        &mut self,
        state: &carosign_engine::OperationState,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<TerminalReservation, ReservationError> {
        use carosign_engine::{EffectSpec, OperationBody, TerminalOutcome};
        if state.common.context.operation_id != self.operation
            || !matches!(state.body, OperationBody::Verification(_))
            || state.common.phase != OperationPhase::Finalizing
            || state.common.terminal != Some(TerminalOutcome::FactorAccepted)
            || state.common.lease.is_some()
            || state.common.cancel_requested
            || state.body.verification().is_none_or(|v| {
                v.delivery != carosign_engine::transition::VerificationDelivery::Pending
            })
            || effect.spec != EffectSpec::DeliverOutcome
            || !state.common.pending_effects.contains(&effect)
        {
            return Err(ReservationError::ContextMismatch);
        }
        if self.reservation.is_some() {
            return Err(ReservationError::Busy);
        }
        if self.terminal_published {
            return Err(ReservationError::Event(EventError::AlreadyTerminal));
        }
        let sequence = self
            .stream
            .last_sequence()
            .checked_add(1)
            .ok_or(ReservationError::Event(EventError::SequenceExhausted))?;
        let reservation_id = std::sync::Arc::new(());
        self.reservation = Some(ReservedTerminal {
            reservation_id: std::sync::Arc::clone(&reservation_id),
            effect,
            sequence,
            stage: ReservationStage::Ready,
        });
        Ok(TerminalReservation {
            identity: self.identity(),
            reservation_id,
            operation: self.operation,
            effect,
            sequence,
        })
    }

    fn reserved_mut(
        &mut self,
        reservation: &TerminalReservation,
    ) -> Result<&mut ReservedTerminal, ReservationError> {
        if reservation.operation != self.operation || !self.matches_identity(&reservation.identity)
        {
            return Err(ReservationError::ContextMismatch);
        }
        let entry = self
            .reservation
            .as_mut()
            .ok_or(ReservationError::ContextMismatch)?;
        if !std::sync::Arc::ptr_eq(&entry.reservation_id, &reservation.reservation_id)
            || entry.effect != reservation.effect
            || entry.sequence != reservation.sequence
        {
            return Err(ReservationError::ContextMismatch);
        }
        Ok(entry)
    }

    /// Marks the beginning of a single attempt at system recording.
    pub fn begin_reserved_send(
        &mut self,
        reservation: &TerminalReservation,
    ) -> Result<(), ReservationError> {
        let entry = self.reserved_mut(reservation)?;
        if entry.stage != ReservationStage::Ready {
            return Err(ReservationError::Busy);
        }
        entry.stage = ReservationStage::Sending;
        Ok(())
    }

    /// Accepts the verified result of a single recording call.
    pub fn finish_reserved_send(
        &mut self,
        reservation: &TerminalReservation,
        outcome: ReservedSendOutcome,
    ) -> Result<(), ReservationError> {
        let entry = self.reserved_mut(reservation)?;
        if entry.stage != ReservationStage::Sending {
            return Err(ReservationError::ContextMismatch);
        }
        entry.stage = match outcome {
            ReservedSendOutcome::NotSent => ReservationStage::Ready,
            ReservedSendOutcome::Sent => ReservationStage::Sent,
            ReservedSendOutcome::Unknown => ReservationStage::Uncertain,
        };
        Ok(())
    }

    /// Cancellates only a reserve for which there is no record.
    pub fn cancel_reserved_terminal(
        &mut self,
        reservation: &TerminalReservation,
    ) -> Result<(), ReservationError> {
        if self.reserved_mut(reservation)?.stage != ReservationStage::Ready {
            return Err(ReservationError::Busy);
        }
        self.reservation = None;
        Ok(())
    }

    /// Publishes the same number only after confirmed shipment and terminal.
    pub fn publish_reserved_terminal(
        &mut self,
        reservation: &TerminalReservation,
        state: &carosign_engine::OperationState,
    ) -> Result<Vec<u32>, ReservationError> {
        use carosign_engine::{OperationBody, TerminalOutcome};
        if self.reserved_mut(reservation)?.stage != ReservationStage::Sent
            || state.common.context.operation_id != self.operation
            || !matches!(state.body, OperationBody::Verification(_))
            || state.common.phase != OperationPhase::Terminal
            || state.common.terminal != Some(TerminalOutcome::FactorAccepted)
            || state.common.lease.is_some()
            || !state.common.pending_effects.is_empty()
            || state.body.verification().is_none_or(|v| {
                v.evidence.is_some()
                    || v.delivery != carosign_engine::transition::VerificationDelivery::Sent
            })
        {
            return Err(ReservationError::ContextMismatch);
        }
        self.reservation = None;
        let disconnected = self
            .publish(OperationPhase::Terminal, EventKind::Terminal)
            .map_err(ReservationError::Event)?;
        if self.stream.last_sequence() != reservation.sequence {
            return Err(ReservationError::ContextMismatch);
        }
        Ok(disconnected)
    }

    /// Publishes a mutation marker only after reading its stable result.
    ///
    /// A separate authorization of each observer is carried out at the time of issuance.
    pub async fn publish_confirmed_mutation_terminal(
        &mut self,
        state: &carosign_engine::OperationState,
        storage: &dyn crate::operation::observer::TerminalOutcomePort,
    ) -> Result<(carosign_engine::TerminalOutcome, Vec<u32>), TerminalPublishError> {
        if state.common.context.operation_id != self.operation || !state.common.phase.is_terminal()
        {
            return Err(TerminalPublishError::ContextMismatch);
        }
        let mutation = state
            .mutation()
            .ok_or(TerminalPublishError::ContextMismatch)?
            .mutation;
        let outcome = storage
            .read(self.operation, mutation)
            .await
            .map_err(|_| TerminalPublishError::Unavailable)?;
        if !matches!(
            outcome,
            carosign_engine::TerminalOutcome::MutationCommitted
                | carosign_engine::TerminalOutcome::NoChange
        ) {
            return Err(TerminalPublishError::Unavailable);
        }
        if self.terminal_published {
            // Re-reading confirms the outcome, but does not create a second event.
            return Ok((outcome, Vec::new()));
        }
        let disconnected = self
            .publish(OperationPhase::Terminal, EventKind::Terminal)
            .map_err(TerminalPublishError::Event)?;
        Ok((outcome, disconnected))
    }

    /// Takes an event for one observer without affecting the others.
    pub fn pop(&mut self, id: u32) -> Option<Event> {
        self.observers
            .get_mut(&id)
            .and_then(|(_, queue)| queue.pop())
    }

    /// A story for authorized replay.
    pub fn stream(&self) -> &EventStream {
        &self.stream
    }

    /// Makes the step of the actor of the same operation and publishes the change in the
    /// non-terminal phase.
    ///
    /// Failure of events does not cancel the decision of the engine: the effects remain in
    /// `step` The final result is published separately, after steady completion and
    /// verification of the rights of the recipient.
    pub fn step_actor(
        &mut self,
        actor: &mut crate::operation::actor::OperationActor,
    ) -> Result<ObservedStep, AttachError> {
        if actor.state().common.context.operation_id != self.operation {
            return Err(AttachError::WrongOperation);
        }
        let previous = actor.state().common.phase;
        let step = actor.run_once();
        let notification = match &step {
            crate::operation::actor::Step::Advanced { state, .. }
                if state.common.phase != previous && !state.common.phase.is_terminal() =>
            {
                self.publish(state.common.phase, EventKind::StateChanged)
            }
            _ => Ok(Vec::new()),
        };
        Ok(ObservedStep { step, notification })
    }
}

#[cfg(test)]
#[path = "event_hub.tests.rs"]
mod tests;
