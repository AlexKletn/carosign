use carosign_core::ids::{CommandId, MutationId};
use carosign_runtime::ports::{CommandPlan, EnrollmentAuthorization, MutationChange};

/// Change number for the plan.
pub fn mutate_plan(
    mutation: MutationId,
    command: CommandId,
    authorization: EnrollmentAuthorization,
    change: MutationChange,
) -> CommandPlan {
    CommandPlan::Mutate {
        mutation,
        command,
        authorization,
        change,
    }
}
