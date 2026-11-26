//! Refusal of the mediator: what becomes unclear.
//!
//! ⚠️ Process closure and device security are different facts.
//! After the middleman falls, leasings remain unclear until a separate indication of the
//! device's safe state.

mod after_crash;
mod worker_after_death;

pub use after_crash::AfterCrash;
pub use after_crash::after_crash;
pub use after_crash::uncertain_leases;
pub use worker_after_death::WorkerAfterDeath;
pub use worker_after_death::revoke_by_termination;
