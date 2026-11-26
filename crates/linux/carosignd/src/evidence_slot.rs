//! Check the owner of the material and the test examiner.

#[cfg(test)]
use carosign_evidence::store::EvidenceSlot;
pub(crate) use carosign_evidence::store::{EvidenceLease, EvidenceStore, SlotError};

#[path = "evidence_slot.tests.rs"]
#[cfg(test)]
mod tests;
