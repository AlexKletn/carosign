/// What the cycle did in one step.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CycleStep {
    /// Transition made, effects executed.
    Progressed {
        /// Phase after transition.
        phase: String,
        /// So many effects.
        effects: usize,
    },
    /// There's nothing to do. We're waiting for observation.
    Waiting,
    /// The engine rejected the entrance: it is not “waiting”, but a refusal with a reason.
    Refused(String),
    /// Operation complete.
    Done,
}
