use carosign_core::ids::OperationId;

/// A one-time flow reserve. It doesn't entitle the recipient of the result.
pub struct TerminalReservation {
    pub(super) identity: std::sync::Arc<()>,
    pub(super) reservation_id: std::sync::Arc<()>,
    pub(super) operation: OperationId,
    pub(super) effect: carosign_engine::PlannedEffect,
    pub(super) sequence: u64,
}
impl TerminalReservation {
    /// A number not yet published to observers.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}
