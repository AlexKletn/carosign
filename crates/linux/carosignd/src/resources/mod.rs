//! Port of resources in the service: leasing is provided by an broker, not a service.
//!
//! ⚠️ Rule 09: The set is taken **whole and atomically.** The service does not collect leases
//! from parts or invent resources: it asks and receives an answer.
//!
//! ⚠️ While there's no real broker, the service gets a rejection -- and that's a word
//! of mouth -- you can't substitute empty leasing, and then the operation goes on without the
//! resource, which is exactly the case when the print is taken without permission.

use core::fmt;

use carosign_core::ids::LeaseId;
use carosign_core::resource::ResourceSet;
use carosign_runtime::ports::{
    BoxFuture, QuiesceAssessment, ResourceDenial, ResourceLease, ResourceOwner, ResourcePort,
};
pub use carosign_runtime::scheduler::cleanup::{
    CleanupPortFailure, CleanupScope, QuiescedLease, TrustedCleanupBroker,
};

#[path = "resources.tests.rs"]
#[cfg(test)]
mod tests;

mod no_broker;
pub use no_broker::NoBroker;
mod local_resource_owner;
pub use local_resource_owner::LocalResourceOwner;
mod exit_cleanup;
pub(crate) use exit_cleanup::ExitCleanup;
mod supervisor_cleanup;
pub(crate) use supervisor_cleanup::SupervisorCleanup;
mod resource_set_source;
pub use resource_set_source::ResourceSetSource;
mod mutation_lease_verifier;
pub(crate) use mutation_lease_verifier::MutationLeaseVerifier;

mod relay_scope;
pub(crate) use relay_scope::relay_scope;
mod denial_name;
pub use denial_name::denial_name;
