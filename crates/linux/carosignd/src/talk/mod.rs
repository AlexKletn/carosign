//! Connection conversation: client frame → service → response.
//!
//! ⚠️ Rule 07: Connection ** does not grant rights ** Credents are removed when connected,
//! but each request goes to the source of authority, and you can see it along the way: the
//! frame does not carry "I am allowed."
//!
//! ⚠️ Order: greetings → requests. Bypassing version approval would open up the reception of
//! commands with a different meaning of the fields, so the first message must be a greeting,
//! and the refusal on it closes the connection.
//!
//! ⚠️ There's only a **h of personnel: take, disassemble, give to service, answer. There's no
//! solution.

use core::fmt;

use carosign_core::device::DeviceEpoch;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::IdSource;
use carosign_linux_ipc::codec::{self, ClientRequest};
use carosign_protocol::carosign::client::v1::ClientFrame;
use carosign_protocol::carosign::common::v1 as common;
use carosign_runtime::ports::{PolicyAuthority, TimeSource};
use prost::Message;

use crate::cycle::{self, Handled};
use crate::server::{self, Connection, ConversationEnd, read_frame, write_frame};
use crate::service::Service;

/// Frame limit for reading: one message `SOCK_SEQPACKET` whole.
mod buffer;
use buffer::FRAME_BUFFER;

#[path = "talk.tests.rs"]
#[cfg(test)]
mod tests;

mod ended;
pub use ended::Ended;
mod asked;
pub use asked::Asked;
mod inventory_response;
mod operation_status_response;
mod verification_dispatcher;
pub(crate) use verification_dispatcher::VerificationDispatcher;
mod counted_serve_error;
pub(crate) use counted_serve_error::CountedServeError;

mod conversation;
pub use conversation::converse;
#[allow(unused_imports)]
pub(crate) use conversation::converse_configured;
#[allow(unused_imports)]
pub(crate) use conversation::converse_counted_with_dispatcher;
pub use conversation::converse_with_health;
#[allow(unused_imports)]
pub(crate) use conversation::serve_counted_once;
pub(crate) use conversation::serve_counted_once_with_dispatcher;
mod hello;
use hello::wait_for_hello;
mod response;
use response::answer;
#[cfg(test)]
use response::unsupported;
mod frame_io;
use frame_io::read_one;
use frame_io::send_one;
