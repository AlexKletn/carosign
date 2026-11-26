use super::TransitionIndex;
use carosign_core::ids::CommandId;

/// What do you remember about the sent command?
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CommandRef {
    /// Command ID's in the wire.
    pub id: CommandId,
    /// The transition in which it originated.
    pub transition_index: TransitionIndex,
}
