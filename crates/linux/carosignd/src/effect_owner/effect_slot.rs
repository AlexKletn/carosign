use carosign_engine::PlannedEffect;

use super::effect_state::EffectState;

pub(super) struct EffectSlot {
    pub(super) effect: PlannedEffect,
    pub(super) state: EffectState,
}
