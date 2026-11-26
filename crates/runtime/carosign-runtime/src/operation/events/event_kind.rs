use super::Progress;

/// What the observer sees.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// Change of state.
    StateChanged,
    /// Move: finger location, shooting, waiting.
    Progress(Progress),
    /// Outcome.
    Terminal,
}
