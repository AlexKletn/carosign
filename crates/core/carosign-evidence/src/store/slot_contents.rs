use super::SlotState;
use crate::material::EvidenceMaterialRef;
use carosign_core::operation::context::BootDeadline;

pub(super) struct SlotContents {
    pub(super) reference: EvidenceMaterialRef,
    pub(super) deadline: BootDeadline,
    pub(super) state: SlotState,
}
