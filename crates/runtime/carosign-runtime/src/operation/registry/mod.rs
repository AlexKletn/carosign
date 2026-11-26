//! Operations register, clock and diagnostics.
//!
//! ⚠️ The registry is the only place where operations are searched by a number. It does not
//! hold a lock for the time of the actor's step: the step is outside (document 05, rule 2).
//!
//! ⚠️ The clock comes by port: the deadline created from the system clock will not survive
//! the reboot, and the fake clock allows you to check the expiration without sleep.

mod entry;
mod limits;
mod manual_clock;
mod operation_registry;
mod owner;
mod registry_error;
mod slot;
mod transfer_error;

use entry::Entry;
pub use limits::OPERATIONS_MAX;
pub use manual_clock::ManualClock;
pub use operation_registry::OperationRegistry;
pub use owner::Owner;
pub use registry_error::RegistryError;
use slot::Slot;
pub use transfer_error::TransferError;
