#[derive(Debug)]
pub(super) struct PreflightChanges {
    pub(super) len: u64,
    pub(super) base_visible_len: u64,
    pub(super) bytes: std::collections::BTreeMap<u64, u8>,
}
