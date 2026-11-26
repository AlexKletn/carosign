//! Registration register: reading and CAS-fixing.
//!
//! ⚠️ A fastener is a *one* transaction: the record, index, and log phase.
//! Otherwise, when the cliff breaks, the index and the record will diverge, and the search by
//! area will begin to show what is not there.
//!
//! ⚠️ CAS is coming in a **-generation: you can only replace what was the generation the
//! client named. The outdated generation is giving up before sending (scenario T5), not
//! taking what's closer.

use core::fmt;

use carosign_core::enrollment::EnrollmentRef;
use carosign_core::ids::EnrollmentId;

use crate::enrollment_codec::EnrollmentRecord;
use crate::model::StorageError;
use crate::store::{MetadataStore, ReadTransaction, Table};

fn decode_record(
    tx: &dyn ReadTransaction,
    key: &[u8],
    bytes: &[u8],
) -> Result<EnrollmentRecord, StorageError> {
    let schema = tx.get(Table::Meta, crate::redb::backend::SCHEMA_KEY)?;
    if matches!(
        schema.as_deref(),
        Some(b"BSC1\0\0\0\x04\0\0\0\0" | b"BSC1\0\0\0\x05\0\0\0\0" | b"BSC1\0\0\0\x06\0\0\0\0")
    ) {
        let legacy = crate::installation_records::checked_v4_record_as_v3(
            crate::installation_records::MigratableRecord::Enrollment,
            key,
            bytes,
        )
        .map_err(|_| StorageError::Corrupt {
            what: "запись реестра v4 не разбирается",
        })?;
        crate::enrollment_codec::decode_v3(&legacy).map_err(|_| StorageError::Corrupt {
            what: "запись реестра v4 не разбирается",
        })
    } else {
        crate::enrollment_codec::decode(bytes).map_err(|_| StorageError::Corrupt {
            what: "запись реестра не разбирается",
        })
    }
}

mod lookup;
pub use lookup::Lookup;
mod registry_reader;
pub use registry_reader::RegistryReader;
