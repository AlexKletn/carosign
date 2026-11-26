use super::CommandRef;
use super::OperationPhase;
use super::TerminalOutcome;
use super::TransitionIndex;
use super::WorkerRef;
use crate::input::PlannedEffect;
use carosign_core::ids::LeaseId;
use carosign_core::operation::context::OperationContext;
use carosign_core::operation::verification::VerificationFailure;
use core::fmt;

/// The common part of the condition is what an operation of any kind has.
#[derive(Clone, PartialEq, Eq)]
pub struct CommonOperationState {
    /// Context that does not change over the life of the operation.
    pub context: OperationContext,
    pub phase: OperationPhase,
    /// Transition meter.
    pub transition_index: TransitionIndex,
    /// You asked me to cancel.
    pub cancel_requested: bool,
    /// Exodus, if it's already here.
    pub terminal: Option<TerminalOutcome>,
    /// The type of refusal if it specifies the terminal.
    pub failure: Option<VerificationFailure>,
    /// Resources issued (for cleaning).
    pub lease: Option<LeaseId>,
    /// Sent a command, if there was.
    pub command: Option<CommandRef>,
    /// The worker you're talking to.
    pub worker: Option<WorkerRef>,
    /// Transition effects for which there is no confirmation yet.
    pub pending_effects: Vec<PlannedEffect>,
}

impl CommonOperationState {
    /// Initial general part: everything is empty, phase `New`.
    pub fn start(context: OperationContext) -> Self {
        Self {
            context,
            phase: OperationPhase::New,
            transition_index: TransitionIndex::INITIAL,
            cancel_requested: false,
            terminal: None,
            failure: None,
            lease: None,
            command: None,
            worker: None,
            pending_effects: Vec::new(),
        }
    }
}

impl fmt::Debug for CommonOperationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommonOperationState")
            .field("phase", &self.phase)
            .field("transition_index", &self.transition_index)
            .field("cancel_requested", &self.cancel_requested)
            .field("terminal", &self.terminal)
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}
