//! The initial border of the tunnel to the broker.
//!
//! Matching identifiers does not certify the process and does not grant authority.
//! Channel transmission and certification of both processes is provided by the supervisor.
//! A limited set of events is supported with the original wire checking.

mod command_encoder;
mod event_validation;
mod hello;
mod hello_error;
mod provider_channel;
mod result_validation;
mod tunnel_client;
mod tunnel_error;

pub use command_encoder::TunnelCommandEncoder;
pub use hello::TunnelHello;
pub use hello_error::HelloError;
pub use provider_channel::TunnelProviderChannel;
pub use tunnel_client::TunnelClient;
pub use tunnel_error::TunnelError;
