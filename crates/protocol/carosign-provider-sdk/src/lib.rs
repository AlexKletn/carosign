//! SDK provider: handshake, commands and events on top `carosign.provider.v1`.
//!
//! ⚠️ It's an optional krate: you can write C, Go, or Python in the same diagrams, and
//! there's just a nice wire-wire-wire-wire-wire-wire-wire-wire-wire-wire-wire-wire-wire-no
//! global policy.
//!
//! ⚠️ The SDK doesn't give authority to the provider: it doesn't decide what resources to
//! give out, it doesn't announce hardware guarantees, it can talk over the wire and check
//! what any recipient is obligated to check.

pub mod boot;
pub mod cancellation;
pub mod command;
pub mod context;
pub mod dispatch;
pub mod events;
pub mod handshake;
mod version;
pub mod wire;

pub use context::WorkerContext;
pub use version::SDK_VERSION;
pub use wire::{Transport, TransportError};
