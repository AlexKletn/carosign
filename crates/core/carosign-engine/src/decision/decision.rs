use crate::input::PlannedEffect;
use crate::transition::OperationState;
use core::fmt;

/// I got it back. `decide`.
#[derive(Clone, PartialEq, Eq)]
pub struct Decision {
    /// A new state.
    pub next: OperationState,
    /// Effects that need to be performed in order.
    pub effects: Vec<PlannedEffect>,
}

impl fmt::Debug for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Decision")
            .field("kind", &self.next.kind())
            .field("phase", &self.next.common.phase)
            .field("effects", &self.effects)
            .finish_non_exhaustive()
    }
}
