//! Side of provider: connection, handshake, bridge.
//!
//! ⚠️ The channel is issued by the platform (`socketpair`), the worker does not have an
//! address (document 06), so this is the work with the ** already issued ** channel.

pub mod bridge;
pub mod connection;
pub mod handshake;
pub mod transport;
pub mod tunnel;
