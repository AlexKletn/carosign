//! State Machines: One for Meaning.
//!
//! ⚠️ Every operation lives in its own machine: check, change, worker, device. Mixing is
//! worse than duplication - checking should not know about it. `DispatchArmed`Deletion is
//! about evidence.
//!
//! Just values and pure functions. No input/output, no clock.

pub mod device;
pub mod enrollment;
pub mod removal;
pub mod verification;
pub mod worker;
