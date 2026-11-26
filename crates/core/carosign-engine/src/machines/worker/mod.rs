//! Worker's machine: start, handshake, tether, readiness, cleaning, failure.
//!
//! Table - document 06, section "Handshake and phases".
//!
//! ⚠️ `WorkerReady` ** doesn't mean ** `DeviceAvailable` An worker who is ready to work on
//! undefined hardware does not let him work.

mod worker_event;
mod worker_phase;

pub use worker_event::WorkerEvent;
pub use worker_phase::{WorkerPhase, next_phase, phase_timeout_s};
