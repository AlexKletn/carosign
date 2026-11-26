//! Maps platform results to fprintd.
//!
//! See specifications 07 and 12 for scanner ownership and template custody.

mod fprintd_phase;
mod fprintd_result;
mod fprintd_state;

pub use fprintd_phase::FprintdPhase;
pub use fprintd_result::FprintdResult;
pub use fprintd_result::from_platform;
pub use fprintd_state::FprintdState;

/// Represents an outcome without evidence.
pub mod carosign_evidence_placeholder;
