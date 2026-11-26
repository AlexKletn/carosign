//! The composition of Linux services and the connection of system providers.

use carosign_core::ids::OperationId;
use carosign_runtime::ports::{
    BoxFuture, CommandPlan, DispatchOutcome, DurableMarker, MutationPort, ProviderExecutionPort,
    TimeSource,
};
use std::sync::Arc;

/// Limits the number of simultaneous connections.
mod limits;
use limits::CLIENT_WORKERS;

#[path = "linux_service.tests.rs"]
#[cfg(test)]
mod tests;

mod capability_state;
pub(crate) use capability_state::CapabilityState;
mod mutation_activity;
pub(crate) use mutation_activity::MutationActivity;
mod active_mutation;
pub(crate) use active_mutation::ActiveMutation;
mod linux_health;
use linux_health::LinuxHealth;
mod verification_only;
use verification_only::VerificationOnly;

mod device_readiness;
use device_readiness::device_readiness;
pub(crate) use device_readiness::probe_capabilities;
pub(crate) use device_readiness::probe_declaration;
mod socket_publication;
use socket_publication::publish_client_socket;
mod recovered_fence;
mod resource_recovery;
use resource_recovery::restore_resource_owner;
mod cancellation_request;
mod catalog_health;
mod discovery_dispatcher;
mod discovery_readiness;
mod observation_stream;
mod observed_dispatcher;
mod operation_directory;
mod operation_status;
mod operation_subscription;
mod serve;
pub use serve::{serve, serve_discovered};
