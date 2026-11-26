use super::RecordResult;

pub(super) enum RecordSlot {
    Running,
    Ready(RecordResult),
    Claimed,
}
