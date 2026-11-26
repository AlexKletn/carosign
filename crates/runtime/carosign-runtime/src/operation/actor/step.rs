use carosign_engine::{OperationState, PlannedEffect, TransitionError};
use core::fmt;

/// One step of an actor.
///
/// ⚠️ The state inside. `Box`A copy of the state in each step drives hundreds of bytes in the
/// mail and stack, and you need it only after the transition.
#[derive(Clone, PartialEq, Debug)]
pub enum Step {
    /// The condition has changed, we need to perform the effects.
    Advanced {
        /// Post-step condition.
        state: Box<OperationState>,
        /// What they ask you to do.
        effects: Vec<PlannedEffect>,
    },
    /// The operation is over; we are not doing anything.
    Terminal,
    /// The post office is empty. We're waiting for surveillance.
    Idle,
    /// Entrance is not consistent with condition.
    Rejected(TransitionError),
    /// The queue of control is full: the operation must be closed safely.
    Overflow,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Advanced { state, effects } => {
                write!(
                    f,
                    "переход в {:?}, эффектов {}",
                    state.common.phase,
                    effects.len()
                )
            }
            Self::Terminal => f.write_str("завершена"),
            Self::Idle => f.write_str("почта пуста"),
            Self::Rejected(error) => write!(f, "отвергнуто: {error}"),
            Self::Overflow => f.write_str("очередь управления переполнена"),
        }
    }
}
