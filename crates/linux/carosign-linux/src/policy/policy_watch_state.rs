/// Surveillance status. `Unchanged` It doesn't confirm authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyWatchState {
    /// No change yet.
    Unchanged,
    /// The picture can no longer be used.
    Invalidated,
}
