//! Material of proof: reference and life expectancy.
//!
//! Bytes are held by a separate limited owner. `store`Only the possibilities tied to the
//! operation and the specific issue come out.
//!
//! ⚠️ The link is one-time: after trying to deliver it can not be replayed to another
//! request.

mod evidence_error;
mod evidence_material_ref;
mod owner_identity;

pub use evidence_error::EvidenceError;
pub use evidence_material_ref::EvidenceMaterialRef;
pub(crate) use owner_identity::OwnerIdentity;

/// Proven state - from the engine, so as not to produce the second type.
pub use carosign_engine::VerifiedState as CheckedState;
