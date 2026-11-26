//! Worker Supervisor: Startup, Change of Era, Loss and Cleanup.
//!
//! ⚠️ Rule 06: The death of the **not* process means that the hardware has calmed down.The
//! supervisor after the loss of an worker says "state is unclear" rather than "free."
//!
//! The supervisor manages the process and reports observations. Hardware guarantees require
//! separate evidence.

mod supervisor;
mod supervisor_error;
mod worker_health;
mod worker_slot;

pub use supervisor::Supervisor;
pub use supervisor_error::SupervisorError;
pub use worker_health::WorkerHealth;
pub use worker_slot::WorkerSlot;
