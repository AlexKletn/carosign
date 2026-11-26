//! Server cycle: client frame → service → answer.
//!
//! ⚠️ Rule 07: Connection ** does not grant rights ** Credits are removed when connected, but
//! the decision is made by the source of authority, and you can see it along the way: the
//! frame does not carry "I am allowed."
//!
//! This layer receives and parses frames, dispatches requests to the service, and sends
//! replies.
//!
//! ⚠️ Nana `BeginVerification` answer `OperationAccepted` (Accepted, not coincidental); the
//! result is a separate frame.

use core::fmt;

use carosign_core::device::DeviceEpoch;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, IdSource};
use carosign_linux_ipc::client::session::ClientSession;
use carosign_linux_ipc::codec::{self, ClientRequest};
use carosign_protocol::carosign::common::v1 as common;
use carosign_runtime::ports::TransactionId;
use carosign_runtime::ports::{CallerContext, PolicyAuthority, TimeSource};
use prost::Message as _;

use crate::answer::{Reply, encode, terminal_frame};
use crate::service::{Served, Service};

// `prost::Message` necessary `encode_to_vec` It's a client frame.

#[path = "cycle.tests.rs"]
#[cfg(test)]
mod tests;

mod handled;
pub use handled::Handled;
mod cycle_error;
pub use cycle_error::CycleError;

mod verification_cycle;
pub use verification_cycle::handle;
#[cfg(test)]
use verification_cycle::terminal_if_complete;
pub use verification_cycle::verification;
mod caller_identity;
pub(crate) use caller_identity::caller_from_session;
mod authenticator_lookup;
pub use authenticator_lookup::authenticator_of;
mod error_frame;
pub use error_frame::error_frame;
