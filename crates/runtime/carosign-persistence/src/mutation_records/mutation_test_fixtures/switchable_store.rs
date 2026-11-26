use super::switchable_tx::SwitchableTx;
use crate::model::StorageError;
use crate::store::MetadataStore;
use crate::store::ReadTransaction;
use crate::store::WriteTransaction;
use crate::store::tests_support::MemoryStore;

pub(super) struct SwitchableStore {
    pub(super) inner: MemoryStore,
    pub(super) lose_ack: std::sync::atomic::AtomicBool,
}

impl MetadataStore for SwitchableStore {
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError> {
        self.inner.begin_read()
    }
    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError> {
        Ok(Box::new(SwitchableTx {
            inner: self.inner.begin_write()?,
            lose_ack: &self.lose_ack,
        }))
    }
}
