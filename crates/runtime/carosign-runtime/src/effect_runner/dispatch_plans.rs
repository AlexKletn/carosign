use crate::ports::CommandPlan;

/// Plans of sending known to the actor.
///
/// ⚠️ They are assembled where there is a register and permits; the performer only executes
/// them.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DispatchPlans {
    /// Check plan (contains a set of authorized registrations).
    pub verify: Option<CommandPlan>,
    /// Change plan (carries the authorization mode).
    pub mutate: Option<CommandPlan>,
    /// Plan of preparation of call of registration.
    pub prepare_challenge: Option<CommandPlan>,
}

impl DispatchPlans {
    /// Plans for verification.
    pub fn verify(plan: CommandPlan) -> Self {
        Self {
            verify: Some(plan),
            ..Self::default()
        }
    }
}
