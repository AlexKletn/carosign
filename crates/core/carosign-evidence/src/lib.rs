//! Evidence and warranty evaluation: verification, requirements, trust assessment. Three
//! layers are separated on purpose — the provider's approval does not become an assessment.
//!
//! The device -- `docs/specs/engineering/implementation-v06/11-evidence-and-assurance.md`.

pub mod assessment;
pub mod error;
pub mod material;
pub mod policy;
pub mod recipient;
pub mod requirements;
pub mod routing;
pub mod store;
pub mod verifier;
