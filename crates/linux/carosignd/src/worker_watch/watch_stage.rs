#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WatchStage {
    Watching,
    Fenced { leases: usize },
}
