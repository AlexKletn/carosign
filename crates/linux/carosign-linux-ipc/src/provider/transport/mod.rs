//! Time-limited exchange with an worker through an already issued channel.
//!
//! Ownership of the descriptor and the counters is in the same facility. It's not a proxy for
//! launch, permission, or physical security.
//! Blocking calls are performed outside the executioner’s stream of operations.

mod exchange_error;
mod provider_transport;

pub use exchange_error::ExchangeError;
pub use provider_transport::ProviderTransport;
pub(crate) use provider_transport::unnamed_unix_address;
