use super::EvidenceSlot;
use carosign_core::ids::EvidenceId;
use carosign_core::ids::OperationId;

pub(super) struct StoreInner {
    pub(super) entries: std::collections::BTreeMap<OperationId, (EvidenceId, EvidenceSlot)>,
    pub(super) generation: u64,
}
