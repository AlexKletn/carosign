//! Listener and sessions: welcome, greeting, first real conversation.
//!
//! ⚠️ Rule 05, step 7: the listener is published ** after ** recovery. Here it's a sequence
//! of calls, not an order in the comment.
//!
//! ⚠️ Credits (UID/PID/GID) remove the kernel when you connect them, and they are not
//! deduced; they are verified by the source of authority (document 08).

use core::fmt;

use carosign_core::ids::IdSource;
use carosign_linux::ids::KernelIds;
use carosign_linux::sys::{self, PeerUser};
use carosign_linux_ipc::client::session::ClientSession;
use carosign_linux_ipc::credentials::PeerCredentials;
#[cfg(test)]
use carosign_protocol::carosign::client::v1 as client;
use carosign_protocol::carosign::client::v1::ClientFrame;
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::validation::check_client_frame;
use carosign_protocol::version;
use prost::Message;

/// The frame limit for reading.
mod buffer;
use buffer::FRAME_BUFFER;

// Zero ID is not assigned; source failure does not cause panic.

#[path = "server.tests.rs"]
#[cfg(test)]
mod tests;

mod accept_outcome;
pub use accept_outcome::AcceptOutcome;
mod connection;
pub use connection::Connection;
mod counted_connection;
pub(crate) use counted_connection::CountedConnection;
mod owner_disconnect_watch;
pub(crate) use owner_disconnect_watch::OwnerDisconnectWatch;
mod counted_accept_error;
pub(crate) use counted_accept_error::CountedAcceptError;
mod conversation_end;
pub use conversation_end::ConversationEnd;

mod connection_admission;
pub(crate) use connection_admission::accept_counted;
pub use connection_admission::accept_one;
#[cfg(test)]
use connection_admission::fresh_connection_id;
pub use connection_admission::new_connection;
#[cfg(test)]
use connection_admission::register_counted;
mod handshake;
pub use handshake::greet;
mod frames;
use frames::check_frame_size;
pub use frames::read_frame;
pub use frames::write_frame;
