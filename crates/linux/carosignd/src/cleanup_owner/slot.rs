use super::CleanupResult;

pub(super) enum Slot {
    Running,
    Ready(CleanupResult),
    Claimed,
}
