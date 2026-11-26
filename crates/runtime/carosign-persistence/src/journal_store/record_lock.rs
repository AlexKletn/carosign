use carosign_core::ids::MutationId;

use crate::model::{MutationRecord, StorageError};
use crate::store::Table;

pub(super) fn check_record_lock(
    tx: &dyn crate::store::WriteTransaction,
    mutation: MutationId,
    record: &MutationRecord,
) -> Result<(), StorageError> {
    if record.mutation != mutation {
        return Err(StorageError::Corrupt {
            what: "ключ изменения не совпадает с записью",
        });
    }
    if tx
        .get(Table::MutationScopeLock, &record.scope_key())?
        .as_deref()
        != Some(mutation.as_bytes().as_slice())
    {
        return Err(StorageError::Corrupt {
            what: "блокировка области отсутствует или принадлежит другому изменению",
        });
    }
    Ok(())
}
