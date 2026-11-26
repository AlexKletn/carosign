//! Descriptors: what can be given and what to do if you refuse.
//!
//! ⚠️ The rule that is more expensive than others (document 09): *the descriptor given
//! remains available** even after the broker has closed its copy.
//! So "withdraw leasing"  ⁇  "take back access." Only the completion of the process takes
//! away access, and physical security proves the reset or driver certificate.
//!
//! ⚠️ At any discrepancy (no slots, an extra descriptor, someone else's role) close ** all
//! the arriving descriptors.

mod fd_plan_error;
mod handoff;
mod handoff_plan;
mod revocation;

pub use fd_plan_error::FdPlanError;
pub use handoff::Handoff;
pub use handoff_plan::HandoffPlan;
pub use handoff_plan::plan;
pub use revocation::Revocation;
