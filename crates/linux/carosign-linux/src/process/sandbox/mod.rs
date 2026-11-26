//! Sandbox of the worker: what is limited and what is not.
//!
//! The worker communicates only over the supplied channel. Create it before `exec` and close
//! the worker end in the parent afterward. See specification 09.
//!
//! ⚠️ There is no default network access: it is not yet configured.

mod relaxation;
mod sandbox;
mod sandbox_error;

pub use relaxation::Relaxation;
pub use sandbox::Sandbox;
pub use sandbox_error::SandboxError;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
