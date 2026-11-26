use super::lost_ack_write::LostAckWrite;
use carosign_persistence::model::StorageError;
use carosign_persistence::store::MetadataStore;
use carosign_persistence::store::ReadTransaction;
use carosign_persistence::store::WriteTransaction;
use carosign_persistence::store::tests_support::MemoryStore;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub(super) struct LostAckStore {
    pub(super) inner: MemoryStore,
    pub(super) lose_ack: AtomicBool,
}

impl MetadataStore for LostAckStore {
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError> {
        self.inner.begin_read()
    }
    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError> {
        Ok(Box::new(LostAckWrite {
            inner: self.inner.begin_write()?,
            lose_ack: &self.lose_ack,
        }))
    }
}
