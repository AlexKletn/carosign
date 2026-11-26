use carosign_core::ids::OperationId;
use carosign_engine::PlannedEffect;

use super::{EffectOwner, effect_state::EffectState};

/// The holder of the effect returns it upon interruption until explicit confirmation.
pub(crate) struct EffectClaim<'a> {
    pub(super) owner: &'a EffectOwner,
    pub(super) id: OperationId,
    pub(super) effect: PlannedEffect,
    pub(super) finished: bool,
    pub(super) started: bool,
}

impl EffectClaim<'_> {
    /// The performer is going to call the external system; on a cliff, a repeat can no longer
    /// be considered safe.
    pub(crate) fn mark_started(&mut self) {
        self.started = true;
    }

    /// Confirms the transfer of effect to the responsible performer.
    pub fn acknowledge(mut self) {
        let mut batches = self
            .owner
            .batches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(batch) = batches.get_mut(&self.id) {
            batch.effects.retain(|slot| slot.effect != self.effect);
            if batch.step_accepted && batch.effects.is_empty() {
                batches.remove(&self.id);
            }
        }
        self.finished = true;
    }
}

impl Drop for EffectClaim<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let mut batches = self
            .owner
            .batches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(slot) = batches.get_mut(&self.id).and_then(|batch| {
            batch
                .effects
                .iter_mut()
                .find(|slot| slot.effect == self.effect)
        }) {
            slot.state = if self.started {
                EffectState::Uncertain
            } else {
                EffectState::Pending
            };
        }
    }
}
