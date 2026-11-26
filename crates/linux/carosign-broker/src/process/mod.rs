//! A private process channel issued by the system supervisor.
//!
//! One operation is allowed without the issuance of hardware descriptors. The changing
//! command requires a one-time confirmation from the private demon channel.

mod active_operation;
mod challenge_preparation;
mod configured_resource;
mod context;
mod control_handler;
mod handshake;
mod owner_revocation;
mod packet_deadline;
mod provider_event;
mod provider_setup;
mod reply;
mod request_handler;
mod request_validation;
mod resource_binding;
mod run;
mod session_state;

pub use configured_resource::ConfiguredResource;
pub use context::Context;
pub use run::{
    run, run_with_control, run_with_discovered_worker, run_with_resource, run_with_resource_owner,
    run_with_resources,
};
