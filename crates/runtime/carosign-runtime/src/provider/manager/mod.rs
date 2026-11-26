//! Catalog of installed providers and launch decision.
//!
//! ⚠️ Zero installed providers is a **healthy** state, not a mistake (P-12). A platform
//! without providers simply doesn't know anything about this modality, and that's no reason
//! to do it. `Degraded`.
//!
//! ⚠️ Installation and launch are different things: there is only a solution.
//! `carosign-linux`.

mod installed;
mod limits;
mod manager_error;
mod provider_manager;

pub use installed::Installed;
pub use limits::PACKAGES_MAX;
pub use manager_error::ManagerError;
pub use provider_manager::ProviderManager;
