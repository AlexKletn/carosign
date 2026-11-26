//! Exclusive scanner ownership.
//!
//! fprintd and the libfprint worker require exclusive access to the scanner. See specification 12.

mod claim_outcome;
mod owner;

pub use claim_outcome::ClaimOutcome;
pub use owner::Owner;
pub use owner::claim;
pub use owner::release;
