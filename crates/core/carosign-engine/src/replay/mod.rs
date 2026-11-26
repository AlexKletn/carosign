//! Repeat: Assembling a state from recorded transitions.
//!
//! ⚠️ Repeat restores the **decision, not the hardware. You can't know what the machine did from
//! the log; you can only know what was ordered and what was confirmed. Inventing the rest is
//! a direct violation of document 04.
//!
//! Determination: The same list of transitions gives the same state (E-07). So there is no
//! time, randomness, or storage-dependent order.

mod rebuild;
mod same_decision;

pub use rebuild::rebuild;
pub use same_decision::same_decision;
