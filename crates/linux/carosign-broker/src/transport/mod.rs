//! Non-blocking transport of a separate broker.
//!
//! Ownership of the socket is not an authority. Prior to sending, the caller is required to
//! check the current policy, issuance, recall and recovery. This module guarantees only the
//! boundary of the packages and the prohibition of descriptors outside Bind.

mod packet_socket;
mod transport_error;

pub use packet_socket::PacketSocket;
pub(crate) use packet_socket::unnamed_unix;
pub use transport_error::TransportError;
