use super::{AdvanceOutcome, DriveOutcome};

#[derive(Debug)]
pub(crate) enum FlowProgress {
    Sent,
    WouldBlock,
    Other(AdvanceOutcome),
    DispatchFailure(DriveOutcome),
}
