//! One exchange check over an already open channel.
//!
//! This facility does not establish trust in the process or verify authority.
//! The result is the worker’s observation, not the confirmation of the factor.

mod pump;

pub(crate) use pump::VerificationContext;
pub use pump::{PumpError, PumpEvent, VerificationPump};

use std::time::Duration;

use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{CommandId, OperationId, WorkerInstanceId};
use carosign_protocol::carosign::provider::v1::{ProviderFrame, provider_frame::Payload};
use carosign_provider_sdk::events::{self, Event};
use carosign_runtime::operation::bindings::{BindingKind, PermittedSet};
use carosign_runtime::provider::session::{ProgressObservation, Session, VerificationObservation};

use carosign_linux_ipc::provider::transport::{ExchangeError, ProviderTransport};

fn tunnel_error(error: carosign_linux_ipc::provider::tunnel::TunnelError) -> ExchangeError {
    use carosign_linux_ipc::provider::tunnel::TunnelError;
    match error {
        TunnelError::Io(error) => ExchangeError::Io(error),
        TunnelError::Timeout => ExchangeError::Timeout,
        TunnelError::Closed => ExchangeError::Closed,
        // The tunnel error may have come after sending; the caller retains an uncertain
        // outcome without classifying it as a failure before sending.
        TunnelError::Invalid => ExchangeError::Io(std::io::Error::other("неверный tunnel")),
    }
}

#[path = "provider_verification.tests.rs"]
#[cfg(test)]
mod tests;

mod verification_phase;
pub use verification_phase::VerificationPhase;
mod verification_error;
pub use verification_error::VerificationError;
mod actor_verification_message;
pub use actor_verification_message::ActorVerificationMessage;
mod actor_verification_error;
pub use actor_verification_error::ActorVerificationError;
mod verification_message;
pub use verification_message::VerificationMessage;
mod received_frame;
use received_frame::ReceivedFrame;
mod verification_exchange;
pub use verification_exchange::VerificationExchange;
mod verification_transport;
pub use verification_transport::VerificationTransport;
