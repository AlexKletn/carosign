use super::EffectPosition;
use super::TransitionIndex;
use carosign_core::ids::OperationId;
use core::fmt;

/// Canonical key of the effect on the disk: a three, not a hash (document 04).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectKey {
    pub operation: OperationId,
    /// A transition in which the effect has occurred.
    pub transition_index: TransitionIndex,
    /// Position in the list of transition effects.
    pub position: EffectPosition,
}

impl fmt::Debug for EffectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectKey")
            .field("transition_index", &self.transition_index)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}
