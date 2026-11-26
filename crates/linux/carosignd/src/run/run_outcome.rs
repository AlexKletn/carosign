use super::{OperationPhase, TerminalOutcome, fmt};

/// How the run ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunOutcome {
    Terminal(TerminalOutcome),
    /// We're waiting for the ISP.
    Waiting {
        /// What phase?
        phase: OperationPhase,
    },
    /// The steps are over and the terminal is gone.
    StepLimit {
        /// What phase?
        phase: OperationPhase,
    },
    /// The engine rejected the entrance: there is no transition with such a phase and such
    /// observation.
    ///
    /// ⚠️ Our word, not “waiting”: refusal and expectation are different troubles, and one
    /// phrase on both sends you to look for the wrong place.
    Refused {
        /// Which phase did the engine fail?
        phase: OperationPhase,
        reason: String,
    },
}

impl fmt::Display for RunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(outcome) => write!(f, "итог: {outcome:?}"),
            Self::Waiting { phase } => write!(f, "ждём на фазе {phase:?}"),
            Self::StepLimit { phase } => write!(f, "шаги кончились на фазе {phase:?}"),
            Self::Refused { phase, reason } => {
                write!(f, "движок отказал на фазе {phase:?}: {reason}")
            }
        }
    }
}
