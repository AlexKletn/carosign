//! Answer to the client’s request: from the frame to the end.
//!
//! ⚠️ Rule of document 07: `OperationAccepted` The answer to the start of the operation says
//! only "accepted and here is the number", and the result comes separately.
//!
//! ⚠️ Refusals sound in their own words: no registrations, the policy banned, could not ask –
//! three different troubles and three different codes of wire.
//!
//! ⚠️ There's no socket here. Receiving and sending `server.rs`Here's what to tell the
//! client.

use core::fmt;

use carosign_core::authorization::GrantScope;
use carosign_core::device::AuthenticatorRef;
use carosign_core::identity::{OwnerRef, SubjectRef};
use carosign_core::ids::{AuthenticatorId, BootId, IdSource, OperationId};
use carosign_core::operation::context::VerificationPurpose;
use carosign_protocol::carosign::client::v1 as client;
use carosign_protocol::carosign::common::v1 as common;
use carosign_runtime::OperationActor;
use carosign_runtime::ports::TimeSource;
use prost::Message;

use crate::accept::{AcceptError, prepare_authorized_verification};

use crate::serve::{Request, actor_for};

#[path = "answer.tests.rs"]
#[cfg(test)]
mod tests;

mod reply;
pub use reply::Reply;
mod request_error;
pub use request_error::RequestError;
mod replay_encoding_error;
pub use replay_encoding_error::ReplayEncodingError;

mod acceptance_error;
pub use acceptance_error::error_of;
mod begin_verification;
pub use begin_verification::begin_verification;
mod verification_reply;
pub use verification_reply::scope_of;
pub(crate) use verification_reply::{terminal_frame, verification_terminal};
mod public_phase;
pub(crate) use public_phase::public_phase;
mod operation_progress_frame;
pub(crate) use operation_progress_frame::operation_progress_frame;
mod capture_feedback;
mod mutation_replay;
pub use mutation_replay::completed_mutation_event;
pub use mutation_replay::completed_mutation_replay_frames;
pub use mutation_replay::completed_mutation_snapshot;
mod encode;
pub use encode::encode;
