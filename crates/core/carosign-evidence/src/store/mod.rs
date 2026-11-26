//! Temporary possession of one material of one operation.
//!
//! It's not a verifier, it's not a source of trust, and the caller has an obligation to
//! independently monitor the deadline and the recall, including while the verifier is holding
//! the material.

mod assertion_lease;
mod evidence_lease;
mod evidence_slot;
mod evidence_store;
mod provider_assertion_record;
mod slot_contents;
mod slot_error;
mod slot_state;
mod store_inner;

pub use assertion_lease::AssertionLease;
pub use evidence_lease::EvidenceLease;
pub use evidence_slot::EvidenceSlot;
pub use evidence_store::EvidenceStore;
pub use provider_assertion_record::ProviderAssertionRecord;
use slot_contents::SlotContents;
pub use slot_error::SlotError;
use slot_state::SlotState;
use store_inner::StoreInner;
