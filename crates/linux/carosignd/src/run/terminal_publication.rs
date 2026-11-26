use super::TerminalOutcome;

/// Post-publication notices and separate owner access to the outcome.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TerminalPublication {
    /// The result is only with the current right of the owner.
    pub owner_outcome: Result<TerminalOutcome, carosign_runtime::ports::AuthorityRefusal>,
    /// Connections that must be closed even after the owner is withdrawn.
    pub disconnected: Vec<u32>,
}
