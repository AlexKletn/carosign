//! Verification through a real supervisor and a separate broker process.
//!
//! Returned surveillance is the provider’s approval, not the accepted factor.

use carosign_core::ids::IdSource;
use carosign_protocol::carosign::{
    common::v1 as common,
    provider::v1::{self as provider, ProviderFrame, provider_frame::Payload},
};
use carosign_runtime::operation::bindings::PermittedSet;
use carosign_runtime::ports::{QuiesceAssessment, ResourceLease, ResourcePort};
use carosign_runtime::provider::session::VerificationObservation;
use std::time::{Duration, Instant};

use crate::provider_supervisor::ProviderSupervisor;
use crate::provider_verification::{
    VerificationExchange, VerificationMessage, VerificationTransport,
};
use crate::resources::{CleanupScope, LocalResourceOwner, relay_scope};

#[path = "supervised_verification.tests.rs"]
#[cfg(test)]
mod tests;

mod outcome;
pub use outcome::Outcome;
mod error;
pub use error::Error;

mod deadline;
use deadline::remaining;
mod tunnel_preparation;
pub(crate) use tunnel_preparation::prepare_mutation_tunnel;
pub use tunnel_preparation::prepare_tunnel;
use tunnel_preparation::prepare_tunnel_until;
mod verification_exchange;
pub use verification_exchange::verify;
