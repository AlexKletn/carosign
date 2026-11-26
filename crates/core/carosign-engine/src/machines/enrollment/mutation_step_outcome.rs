use crate::input::EffectSpec;
use crate::transition::OperationPhase;
use crate::transition::TerminalOutcome;

/// The result of the step of the change machine.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MutationStepOutcome {
    /// New phase.
    pub phase: OperationPhase,
    /// What do you want me to do?
    pub effects: Vec<EffectSpec>,
    /// The outcome, if it's already known.
    pub terminal: Option<TerminalOutcome>,
    /// Activity with the registry when fixing.
    pub registry: Option<crate::reconcile::RegistryAction>,
    /// The resources provided by this step.
    pub lease_taken: Option<carosign_core::ids::LeaseId>,
    /// The command sent by this step.
    pub command_sent: Option<crate::transition::CommandRef>,
}

impl MutationStepOutcome {
    pub(super) fn new(phase: OperationPhase, effects: Vec<EffectSpec>) -> Self {
        Self {
            phase,
            effects,
            terminal: None,
            registry: None,
            lease_taken: None,
            command_sent: None,
        }
    }

    pub(super) fn terminal(
        phase: OperationPhase,
        effects: Vec<EffectSpec>,
        outcome: TerminalOutcome,
    ) -> Self {
        Self {
            phase,
            effects,
            terminal: Some(outcome),
            registry: None,
            lease_taken: None,
            command_sent: None,
        }
    }
}
