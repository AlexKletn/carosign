//! Resources: Adapter to the broker.
//!
//! ⚠️ Rule 09: **Leasing is provided by an broker, not an adapter.
//! The adapter translates the domain set into the application and back; no decision is made.
//!
//! ⚠️ Atomicity persists: either the whole set or failure. Partial output is impossible to
//! build.

mod broker_client;
mod grant_outcome;
mod resource_bridge;

pub use broker_client::BrokerClient;
pub use grant_outcome::GrantOutcome;
/// Adapter to the resource broker.
pub use resource_bridge::ResourceBridge as BrokerResourcePort;
pub use resource_bridge::ResourceBridge;
