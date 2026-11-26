use super::switchable_tx::SwitchableTx;
use crate::model::StorageError;
use crate::store::MetadataStore;
use crate::store::ReadTransaction;
use crate::store::WriteTransaction;

pub(super) struct LostAckFile<'a> {
    pub(super) inner: &'a crate::redb::backend::RedbStore,
    pub(super) lose_ack: std::sync::atomic::AtomicBool,
}

impl MetadataStore for LostAckFile<'_> {
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
