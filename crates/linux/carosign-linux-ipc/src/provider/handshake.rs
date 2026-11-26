//! Handshake with the worker on the platform side.
//!
//! ⚠️ Expectations: greeting 5c, agreeing 5c, binding 15c (document 06) The breach of term is
//! closing the worker, not waiting.

pub use carosign_provider_sdk::handshake::{Phase, PhaseViolation, accepts};

/// Phase waiting time, second.
pub fn timeout_s(phase: Phase) -> Option<u8> {
    use carosign_core::error::limits;
    match phase {
        Phase::Starting | Phase::Negotiating => Some(limits::PROVIDER_HANDSHAKE_TIMEOUT_S),
        Phase::Binding => Some(limits::PROVIDER_BIND_TIMEOUT_S),
        _ => None,
    }
}
