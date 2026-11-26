use crate::model::StorageError;

pub(super) fn corrupt() -> StorageError {
    StorageError::Corrupt {
        what: "неверная запись изменения v3",
    }
}
