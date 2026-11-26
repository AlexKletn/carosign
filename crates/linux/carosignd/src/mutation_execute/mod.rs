//! Running a change through an actor, a stable journal, and a separate mediator.

mod active_mutation;
mod actor;
mod challenged_enrollment;
mod enrollment_authorization;
mod enrollment_challenge;
mod failure;
mod observed_mutation;
mod reception;
mod resource_cleanup;
mod start;

use crate::mutation_dispatch::{MutationRequest, PreparedMutation};
use active_mutation::ActiveMutation;
use actor::{actor_advance, actor_effect};
use enrollment_authorization::EnrollmentAuthorizationGuard;
use enrollment_challenge::enrollment_challenge_frame;
pub(crate) use failure::MutationStartError;
use observed_mutation::ObservedMutation;
pub(crate) use observed_mutation::finish_guarded;
pub(crate) use reception::MutationReception;
use resource_cleanup::release_mutation_resources;
pub(crate) use start::start;

#[cfg(test)]
use enrollment_challenge::enrollment_challenge_observation;
#[cfg(test)]
use observed_mutation::finish;
#[cfg(test)]
use start::start_inner;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
