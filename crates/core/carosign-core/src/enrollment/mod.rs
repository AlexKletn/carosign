//! Registration: generation, label, opaque link provider.
//!
//! ⚠️ Generation`EnrollmentGeneration`It starts with **1** and grows only by proven addition.
//! Removal with an older generation is rejected by the **up-sending command (Scenario T5), so
//! generational conflict is not a user error, it's an honest rejection.
//!
//! There are no template bytes in this module: neither open nor encrypted.
//! Storage - a separate krate `carosign-persistence`.

mod enrollment_generation;
mod enrollment_label;
mod enrollment_ref;
mod invalid_generation;
mod invalid_label;
mod invalid_opaque_ref;
mod opaque_provider_ref;

pub use enrollment_generation::EnrollmentGeneration;
pub use enrollment_label::{ENROLLMENT_LABEL_MAX, EnrollmentLabel};
pub use enrollment_ref::EnrollmentRef;
pub use invalid_generation::InvalidGeneration;
pub use invalid_label::InvalidLabel;
pub use invalid_opaque_ref::InvalidOpaqueRef;
pub use opaque_provider_ref::{OPAQUE_REFERENCE_MAX, OpaqueProviderRef};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
