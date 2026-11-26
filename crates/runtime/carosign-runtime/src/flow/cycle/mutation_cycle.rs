use crate::ports::CallerContext;
use crate::ports::CommandPlan;
use crate::ports::MutationChange;
use carosign_core::authorization::GrantScope;

/// Change plan for the registration cycle.
///
/// ⚠️ There's no assemblies of projection: it gives it. `persistence::projection` The cycle
/// gets the change that's already been tested.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MutationCycle {
    /// Who asks?
    pub caller: CallerContext,
    /// What he asks.
    pub scope: GrantScope,
    /// Verified change.
    pub change: MutationChange,
    pub mutation: carosign_core::ids::MutationId,
    /// Command number.
    pub command: carosign_core::ids::CommandId,
    /// Authorization mode.
    pub authorization: crate::ports::EnrollmentAuthorization,
}

impl MutationCycle {
    /// The mutation command's plan.
    pub fn plan(&self) -> CommandPlan {
        CommandPlan::Mutate {
            mutation: self.mutation,
            command: self.command,
            authorization: self.authorization,
            change: self.change.clone(),
        }
    }
}
