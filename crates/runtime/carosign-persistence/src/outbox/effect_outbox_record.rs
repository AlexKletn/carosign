use super::{CommandId, EffectKey, EffectPhase, MutationId};

/// Data data `EffectOutboxRecord` from the storage scheme.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectOutboxRecord {
    /// Exact effect address.
    pub key: EffectKey,
    /// Provider command, if assigned.
    pub command: Option<CommandId>,
    /// The effect phase.
    pub phase: EffectPhase,
    /// Change if the effect is related to mutation.
    pub mutation: Option<MutationId>,
}

impl EffectOutboxRecord {
    /// Key Redb: Transaction, Transition, and Position in Order from Document 23.
    pub fn storage_key(&self) -> [u8; crate::limits::OUTBOX_KEY_LEN] {
        crate::redb::codec::outbox_key(
            self.key.operation.as_bytes(),
            self.key.transition_index.value(),
            self.key.position.value(),
        )
    }
}
