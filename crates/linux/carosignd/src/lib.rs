//! The root of platform assembly: settings, recovery, listener, sessions.
//!
//! ⚠️ There are no state domain machines here - only wiring (document 02,
//! `carosignd::assemble`Engines, ports and adapters make decisions.
//!
//! ⚠️ Launch order (document 05): settings → storage → recovery → posting listener. reverse
//! order is a service that lets customers in before it knows which areas are closed.

pub mod accept;
pub mod answer;
pub mod assemble;
pub mod bootstrap;
mod broker_control;
pub(crate) mod cancel_reader;
pub mod cleanup_owner;
pub mod config;
#[allow(dead_code)]
pub(crate) mod control_watch;
pub mod cycle;
#[allow(dead_code)]
pub(crate) mod effect_owner;
mod enrollment_connector;
#[allow(dead_code)]
pub(crate) mod evidence_slot;
pub mod health;
#[allow(dead_code)]
pub(crate) mod install_quarantine;
pub mod installation;
pub mod inventory;
#[cfg(test)]
#[path = "linux_service.adapters.tests.rs"]
mod linux_adapter_e2e;
#[cfg(test)]
#[path = "linux_dispatch.cancellation.tests.rs"]
mod linux_cancel_test;
pub(crate) mod linux_dispatch;
#[cfg(test)]
#[path = "linux_service.mutations.tests.rs"]
mod linux_mutation_e2e;
pub mod linux_policy;
pub mod linux_service;
#[allow(dead_code)]
pub(crate) mod mutation_control;
#[allow(dead_code)]
pub(crate) mod mutation_delivery;
#[allow(dead_code)]
pub(crate) mod mutation_dispatch;
mod rename_enrollment;
pub(crate) mod mutation_execute;
pub mod operations;
#[allow(dead_code)]
pub(crate) mod policy_authority;
mod provider_catalog;
pub mod provider_supervisor;
pub mod provider_verification;
pub mod resources;
pub mod run;
pub(crate) mod scoped_challenge;
#[allow(dead_code)]
pub(crate) mod scoped_verifier;
pub mod serve;
pub mod server;
pub mod service;
pub mod shutdown;
pub mod supervised_verification;
pub mod talk;

#[allow(dead_code)]
pub(crate) mod worker_watch;

#[allow(dead_code)]
pub(crate) mod verification_owner;

#[allow(dead_code)]
pub(crate) mod verification_idempotency;

#[cfg(test)]
#[path = "service/service.completion.tests.rs"]
mod service_completion_tests;

#[cfg(test)]
mod policy_test_lock;
#[cfg(test)]
pub(crate) use policy_test_lock::policy_test_lock;
