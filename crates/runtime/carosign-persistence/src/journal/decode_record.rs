use crate::model::{MutationRecord, StorageError};

/// Disassembles the log entry from the meaning.
pub fn decode_record(bytes: &[u8]) -> Result<MutationRecord, StorageError> {
    // For now, only the temporal format is read; this is not a migration to the v3 scheme.
    crate::redb::record::decode_mutation(bytes)
}
