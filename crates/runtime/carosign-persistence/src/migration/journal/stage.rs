use super::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Stage {
    Pending,
    Staged(Snapshot),
    Committed(Snapshot),
}
