//! Operation server: from the request of the client to the end - by steps.
//!
//! ⚠️ Rule of Document 05/22: The state of the operation is changed by the actor only; here
//! is the order around him: collect context, create the actor, chase the actor.
//! `Gateway::step`, to sum up.
//!
//! Runs a request through the operation actor and its effects to a terminal outcome. Socket
//! handling and wire encoding live in the server and Linux IPC layers.

use core::fmt;

use carosign_core::device::AuthenticatorRef;
use carosign_core::identity::{OwnerRef, SubjectRef};
use carosign_core::ids::IdSource;
use carosign_core::operation::context::{BootDeadline, OperationContext, VerificationPurpose};
use carosign_runtime::OperationActor;
use carosign_runtime::ports::TimeSource;

/// Preparing an actor for a request.
///
/// ⚠️ The security clearance is here: asks him `accept::prepare_verification`.
/// It's just the context and the actor, so that the denial of authority sounds before the
/// operation happens.
pub fn actor_for(request: &Request, context: OperationContext) -> OperationActor {
    let _ = request;
    OperationActor::new(carosign_engine::OperationState::verification(context))
}

/// Sends clearance to the actor.
///
/// ⚠️ The refusal is visible immediately: if the mail does not accept, the operation does not
/// begin.
pub fn admit(actor: &mut OperationActor) -> Result<(), ServeError> {
    actor
        .post(carosign_runtime::OperationMsg::Domain(
            carosign_engine::DomainInput::Authorized,
        ))
        .map_err(|_| ServeError::Refused)
}

#[path = "serve.tests.rs"]
#[cfg(test)]
mod tests;

mod serve_error;
pub use serve_error::ServeError;
mod request;
pub use request::Request;
