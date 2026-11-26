//! Delivery of proof: one-time, to its recipient.
//!
//! ⚠️ The material belongs to one transaction, one entity, one target and one recipient
//! (document 11) This type holds only the local state of the reference; it is not a registry
//! of issue and does not clear the material.

mod held;
mod material_state;
mod routing_error;

pub use held::Held;
pub use material_state::MaterialState;
pub use routing_error::RoutingError;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
