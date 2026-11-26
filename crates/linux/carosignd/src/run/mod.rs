//! The cycle of operation to the end: the steps of the actor and the execution of effects.
//!
//! ⚠️ Rule 05: Effects are performed by the performer, and observations are returned to the
//! actor, and here the circle closes: step → effects → observations → next step until the
//! terminal arrives.
//!
//! The cycle runs the operation and returns its outcome. The server owns sockets and result
//! serialization.
//!
//! ⚠️ If there is no observation, the next step is not taken: waiting is a legal state, and
//! you cannot invent confirmation.

use core::fmt;

use carosign_core::authorization::{AuthorizationBoundary, ValidatedGrant};
use carosign_engine::{EffectReceipt, EffectSpec, OperationPhase, TerminalOutcome};
use carosign_runtime::OperationActor;
use carosign_runtime::effect_runner::{EffectRunner, Execution};
use carosign_runtime::operation::actor::Step;
use carosign_runtime::ports::PolicyAuthority;

mod limits;
pub use limits::STEPS_MAX;

mod observation;
pub use observation::Observation;

mod event_notification;
pub use event_notification::EventNotification;

#[path = "run.tests.rs"]
#[cfg(test)]
mod tests;

mod run_outcome;
pub use run_outcome::RunOutcome;
mod observed_execution;
pub use observed_execution::ObservedExecution;
mod terminal_publication_error;
pub use terminal_publication_error::TerminalPublicationError;
mod terminal_publication;
pub use terminal_publication::TerminalPublication;

mod grant_binding;
pub(crate) use grant_binding::grant_matches_operation;
mod effect_runner;
pub use effect_runner::next_effect;
pub use effect_runner::run;
use effect_runner::run_inner;
pub use effect_runner::steps_to_send;
pub use effect_runner::waits_for_provider;
mod observed_runner;
pub use observed_runner::publish_observed_mutation_terminal;
pub use observed_runner::run_observed;
pub use observed_runner::run_observed_with_storage;
