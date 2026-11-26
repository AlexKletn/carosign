#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) struct Snapshot {
    pub count: u64,
    pub digest: [u8; 32],
}
