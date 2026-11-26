/// What happened to the operation ** in user terms **.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TerminalOutcome {
    /// Factor accepted within this request (not login).
    FactorAccepted,
    NotMatched,
    /// Couldn't get it off.
    NoSample,
    Cancelled,
    /// The machine's unavailable.
    Unavailable,
    /// Refusal.
    Failed,
    /// The change is fixed.
    MutationCommitted,
    /// There was no change.
    NoChange,
    /// The change could have happened; a reconciliation is needed.
    NeedsReconciliation,
}
