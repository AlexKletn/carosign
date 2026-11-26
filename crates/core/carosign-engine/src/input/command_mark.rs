use crate::transition::TransitionIndex;
use carosign_core::ids::CommandId;

/// Sent command mark: number and transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CommandMark {
    /// Command number's on the wire.
    pub command: CommandId,
    /// In what transition did it originate?
    pub transition: TransitionIndex,
}
