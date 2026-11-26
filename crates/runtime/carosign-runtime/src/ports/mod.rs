//! Runtime ports: Everything that requires a system comes from the outside.
//!
//! ⚠️ Runtime **n't import **n't import **n't `carosign-linux-ipc`He sees only those traits;
//! who is behind them decides the root of the build.`carosignd`).
//!
//! ⚠️ No port gives out trusted values: `ValidatedGrant`, `AssuranceAssessment`,
//! `ResourceLease` They come only from incarnations that are inside a trusted part of the
//! platform. Bytes from the wire don't become bytes from the wire.

mod authority;
mod caller_context;
mod evidence;
mod future;
mod mutation_arm;
mod mutation_journal;
mod provider_execution;
mod resources;
mod time;

pub use authority::{AuthorityRefusal, IdentityAuthority, PolicyAuthority};
pub use caller_context::{CallerContext, TransactionId};
pub use evidence::{
    EvidenceContext, EvidencePort, EvidenceRefusal, ScopedAssertionAssessment,
    ScopedAssertionContext,
};
pub use future::BoxFuture;
pub use mutation_arm::{MutationArmFailure, MutationArmPort, MutationArmRequest};
pub use mutation_journal::{DurableMarker, MutationPort};
pub use provider_execution::{
    CommandPlan, DispatchOutcome, EnrollmentAuthorization, MutationChange, ProviderExecutionPort,
};
pub use resources::{
    QuiesceAssessment, ResourceDenial, ResourceLease, ResourceOwner, ResourcePort,
};
pub use time::TimeSource;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod mutation_arm_tests;
