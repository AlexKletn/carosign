use carosign_runtime::operation::actor::Step;

use super::effect_slot::EffectSlot;

pub(super) struct Batch {
    pub(super) step: Step,
    pub(super) step_accepted: bool,
    pub(super) effects: Vec<EffectSlot>,
}
