//! Runs operation actors, dispatches effects, and coordinates resources and provider processes.
//!
//! The engine supplies pure state transitions. Runtime executes their effects and feeds
//! observations back through ports. Linux IPC and hardware-specific types stay outside runtime.
//!
//! Actor steps and mailbox parsing are synchronous. Tests supply time through a port. The
//! effect runner exposes an async interface without depending on Tokio.

pub mod audit;
pub mod authority;
pub mod builder;
pub mod diagnostics;
pub mod dispatcher;
pub mod effect_runner;
pub mod enrollment_authorization;
pub mod enrollment_admission;
pub mod flow;
pub mod handle;
pub mod operation;
pub mod ports;
pub mod provider;
pub mod recovery;
pub mod scheduler;
pub mod services;
pub mod time;

pub use flow::cycle::{CycleError, CycleStep, MutationCycle, Stage, VerificationCycle, step_actor};
pub use operation::actor::{OperationActor, OperationMsg, Step};
pub use operation::bindings::{
    BindingError, BindingKind, EnrollmentBinding, PermittedSet, SetError,
};
pub use operation::registry::{ManualClock, OperationRegistry};
pub use ports::{
    BoxFuture, CallerContext, CommandPlan, DispatchOutcome, DurableMarker, EvidencePort,
    IdentityAuthority, MutationPort, PolicyAuthority, ProviderExecutionPort, QuiesceAssessment,
    ResourceDenial, ResourceLease, ResourceOwner, ResourcePort, TimeSource, TransactionId,
};
