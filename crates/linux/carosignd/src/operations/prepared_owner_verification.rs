use carosign_core::ids::OperationId;

/// A single shot of a negative result.
/// The number has already been assigned to the event stream; the socket entry has not yet
/// been confirmed.
pub struct PreparedOwnerVerification {
    pub(super) operation: OperationId,
    pub(super) outcome: carosign_engine::TerminalOutcome,
    pub(super) sequence: u64,
}

impl PreparedOwnerVerification {
    /// It absorbs the picture; the sending remains the caller.
    pub fn into_parts(self) -> (OperationId, carosign_engine::TerminalOutcome, u64) {
        (self.operation, self.outcome, self.sequence)
    }
}
