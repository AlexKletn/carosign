//! The context of the operation: who, on what, for what and until what time.
//!
//! ⚠️ Deadline lives only while the machine is working: it carries a monotonous moment
//! and`BootId`](crate::ids::BootId)The deadline cannot survive the reboot - the check after
//! the restart begins again, and does not continue on the old clock (document 05).

mod boot_deadline;
mod boot_instant;
mod invalid_purpose;
mod operation_context;
mod verification_purpose;

pub use boot_deadline::BootDeadline;
pub use boot_instant::BootInstant;
pub use invalid_purpose::InvalidPurpose;
pub use operation_context::OperationContext;
pub use verification_purpose::VerificationPurpose;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
