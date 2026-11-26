use super::lost_arm_ack_transaction::Transaction;
use crate::model::StorageError;
use crate::store::MetadataStore;
use crate::store::ReadTransaction;
use crate::store::WriteTransaction;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

pub(super) struct LostArmAck<'a> {
    pub(super) store: &'a crate::redb::backend::RedbStore,
    pub(super) lose_ack: AtomicBool,
}

impl MetadataStore for LostArmAck<'_> {
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError> {
        self.store.begin_read()
    }
    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError> {
        Ok(Box::new(Transaction {
            inner: self.store.begin_write()?,
            lose_ack: self.lose_ack.load(Ordering::SeqCst),
        }))
    }
}
