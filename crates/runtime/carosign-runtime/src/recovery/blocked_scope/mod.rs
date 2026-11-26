//! Fencing areas and retention of restored operations.
//!
//! It is a memory of the current process, not a replacement for a stable log lock.
//! The fence can be removed only through the not yet connected finalization.

mod entry;
mod limits;
mod recovery_coordinator;
mod recovery_error;
mod recovery_scope;
mod registration_failure;

use entry::Entry;
pub use limits::RECOVERY_SCOPES_MAX;
pub use recovery_coordinator::RecoveryCoordinator;
pub use recovery_error::RecoveryError;
pub use recovery_scope::RecoveryScope;
pub use registration_failure::RegistrationFailure;
