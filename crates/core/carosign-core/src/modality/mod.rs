//! Modality and type of operation.
//!
//! ⚠️ In baseline v1, the modality is exactly the same — the fingerprint. Adding a new look
//! (face, voice) changes questions to personality and authority, so it requires a separate
//! ADR, not a new line in the list.
//!
//! `Identify` ("Who is this?") in v1 ** is deliberately absent** (document 03): it reveals
//! the identity instead of proving the one already named.

mod modality;
mod operation_kind;
mod unknown_modality;

pub use modality::Modality;
pub use operation_kind::OperationKind;
pub use unknown_modality::UnknownModality;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
