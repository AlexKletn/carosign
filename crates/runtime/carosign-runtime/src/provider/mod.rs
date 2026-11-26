//! Providers: packages, catalog, selection, supervisor and session.
//!
//! ⚠️ Runtime doesn't run processes itself: it asks for it at the port.`carosign-linux`).
//! It's just the decisions — what's established, what's appropriate, what's ambiguous and
//! what to do if you lose an worker.

pub mod inventory;
pub mod manager;
pub mod packages;
pub mod resolver;
pub mod session;
pub mod supervisor;

pub use manager::{Installed, ProviderManager};
pub use packages::{Candidate, Manifest, ManifestError, Profile, Resolution, resolve};
pub use supervisor::{Supervisor, SupervisorError, WorkerHealth, WorkerSlot};
