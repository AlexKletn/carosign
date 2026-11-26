//! Worker phases and handshake.
//!
//! The session follows `Starting → Negotiating → Binding → Ready → Draining`. Reject
//! out-of-phase messages before taking any action. See specification 06.

mod handshake_step;
mod phase;
mod phase_violation;

pub use handshake_step::HandshakeStep;
pub use phase::Phase;
pub use phase::accepts;
pub use phase::accepts_with_features;
pub use phase::message_name;
pub use phase::transition;
pub use phase_violation::PhaseViolation;
