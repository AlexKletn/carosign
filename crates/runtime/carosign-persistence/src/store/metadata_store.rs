use super::{ReadTransaction, StorageError, WriteTransaction};

/// Warehouse: Can only open transactions.
///
/// ⚠️ There's no bigger api on purpose: anyone could just write it in a table, bypassing a
/// journal.`MutationRepository`](crate::journal::MutationRepository).
pub trait MetadataStore: Send + Sync {
    /// Reading.
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError>;
    /// Record.
    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError>;
}
