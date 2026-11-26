#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReservationStage {
    Ready,
    Sending,
    Sent,
    Uncertain,
}
