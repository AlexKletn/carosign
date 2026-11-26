use crate::model::{MutationRecord, StorageError};
use crate::store::Table;

pub(super) fn check_dispatch_outbox(
    tx: &dyn crate::store::WriteTransaction,
    record: &MutationRecord,
) -> Result<(), StorageError> {
    let command = record.command.ok_or(StorageError::Corrupt {
        what: "нет команды отправленного изменения",
    })?;
    let key = crate::redb::codec::outbox_key(record.operation.as_bytes(), 0, 0);
    if tx.get(Table::EffectOutbox, &key)?.as_deref() != Some(command.as_bytes().as_slice()) {
        return Err(StorageError::Corrupt {
            what: "очередь не соответствует отправленному изменению",
        });
    }
    Ok(())
}
