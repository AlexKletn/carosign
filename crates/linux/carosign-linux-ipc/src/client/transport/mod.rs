//! Public protocol common transport. Connection is not an authority.

mod capture_feedback;
mod client_transport;
mod connection_state;
mod transport_error;

pub use client_transport::Transport;
use connection_state::ConnectionState;
pub use transport_error::TransportError;
