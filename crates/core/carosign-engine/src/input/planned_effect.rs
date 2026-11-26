use super::EffectSpec;
use crate::transition::TransitionIndex;

/// The required effect with your place in the transition is what gets in the queue.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlannedEffect {
    /// What an effect.
    pub spec: EffectSpec,
    /// The transition in which he arose.
    pub transition: TransitionIndex,
    /// Position on the list.
    pub position: u32,
}

impl PlannedEffect {
    /// The effect is exactly at this point of transition.
    pub fn at(spec: EffectSpec, transition: TransitionIndex, position: u32) -> Self {
        Self {
            spec,
            transition,
            position,
        }
    }
}
