//! The ability of the provider: what you can ask, not who to trust.
//!
//! ⚠️ This is a false idea more often than not: `CapabilityDescriptor` The ISP's data is from
//! the wire, and he announced it himself.`AssuranceClass`](crate::assurance::AssuranceClass)
//! So the tester takes it out. `CapabilityDescriptor` You can compare and compare, but you
//! can’t turn it into trust. `ValidatedGrant`/`AssuranceAssessment` They don't grow out of
//! it.
//!
//! Unknown Capacity → **not* invoked (Document 03).

mod capability_descriptor;
mod interaction_mode;
mod recovery_mode;
mod template_custody;
mod unsupported_capability;

pub use capability_descriptor::CapabilityDescriptor;
pub use interaction_mode::InteractionMode;
pub use recovery_mode::RecoveryMode;
pub use template_custody::TemplateCustody;
pub use unsupported_capability::UnsupportedCapability;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
