use super::reservation_stage::ReservationStage;

#[derive(Debug)]
pub(super) struct ReservedTerminal {
    pub(super) reservation_id: std::sync::Arc<()>,
    pub(super) effect: carosign_engine::PlannedEffect,
    pub(super) sequence: u64,
    pub(super) stage: ReservationStage,
}
