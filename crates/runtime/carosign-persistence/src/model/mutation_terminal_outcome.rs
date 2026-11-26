/// Sustainable outcome of completed change (Scheme v3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MutationTerminalOutcome {
    /// The desired state is fixed.
    Committed,
    /// The absence of change is independently confirmed.
    NoChange,
}
