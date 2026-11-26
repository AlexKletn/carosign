pub(super) struct DurableStore(
    pub(super) std::sync::Arc<carosign_persistence::redb::backend::InitialFenceStore>,
);

impl core::fmt::Debug for DurableStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DurableStore(..)")
    }
}
