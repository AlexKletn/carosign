//! Engine: Clean transitions and effects.
//!
//! ⚠️ No **I/O, no clock, no random numbers, no sockets, no protobuf. Observations,
//! identifiers, and powers come back verified.
//!
//! General rule: [`Decision`] says ** where we ended up ** and ** what to ask for ** The
//! engine never invents proof of effect and never thinks the shipment is done.
//!
//! Transitions. `docs/specs/engineering/implementation-v06/04-pure-engine.md`.

#![forbid(unsafe_code)]

pub mod decision;
pub mod input;
pub mod invariant;
pub mod machines;
pub mod reconcile;
pub mod recovery;
pub mod replay;
pub mod transition;

/// Background values for inspections.
///
/// ⚠️ Not for use in the work: the context of the operation is invented here.
#[doc(hidden)]
pub mod tests_support {
    pub use crate::machines::verification::tests_support::*;
}

pub use decision::{Decision, TransitionError, decide};
pub use input::DomainInput;
pub use input::{EffectReceipt, EffectSpec, PlannedEffect};
pub use reconcile::{ReconciliationVerdict, RegistryAction, VerifiedState};
pub use transition::{
    CommandRef, CommonOperationState, EffectPosition, MutationState, OperationBody, OperationPhase,
    OperationState, PhaseFamily, TerminalOutcome, TransitionIndex, VerificationState, WorkerRef,
};
