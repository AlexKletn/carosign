use super::disk_lost_ack_tx::LostAckTx;
use crate::model::StorageError;
use crate::store::MetadataStore;
use crate::store::ReadTransaction;
use crate::store::WriteTransaction;

pub(super) struct LostAck<'a>(pub(super) &'a crate::redb::backend::RedbStore);

impl MetadataStore for LostAck<'_> {
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError> {
        self.0.begin_read()
    }
    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError> {
        Ok(Box::new(LostAckTx(self.0.begin_write()?)))
    }
}
