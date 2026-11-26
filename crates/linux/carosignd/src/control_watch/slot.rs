use super::WatchResult;

pub(super) enum Slot {
    Running,
    Ready(WatchResult),
    Claimed,
}
