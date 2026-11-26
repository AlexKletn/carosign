use crate::model::{MutationRecord, StorageError};
use crate::redb::record::decode_mutation;

use super::ReadSchema;

pub(super) fn read_mutation(
    schema: ReadSchema,
    key: &[u8],
    bytes: &[u8],
) -> Result<MutationRecord, StorageError> {
    match schema {
        ReadSchema::Legacy => decode_mutation(bytes),
        ReadSchema::V3 => crate::mutation_records::decode_mutation(bytes),
        ReadSchema::V4 => {
            let decoded = crate::installation_records::checked_v4_record_as_v3(
                crate::installation_records::MigratableRecord::Mutation,
                key,
                bytes,
            )
            .map_err(|_| StorageError::Corrupt {
                what: "неверная запись изменения v4",
            })?;
            crate::mutation_records::decode_mutation(&decoded)
        }
    }
}
