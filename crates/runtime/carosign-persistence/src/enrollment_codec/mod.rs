//! Analysis of the registry record: `StoredValue.enrollment` from `carosign.storage.v1`.
//!
//! ⚠️ Rule of document 23: value - **shell** `BIO1` + length + protobuf + SHA-256, not the
//! raw structure. The parsing checks the shell, the schema version and the **all** recording
//! fields; the unknown field is not "missing" but failure.
//!
//! ⚠️ `schema_version = 3` for new entries (document 34) Temporary codec `encode`/`decode`
//! uses a non-regulatory amount; individual `encode_v3`/`decode_v3` There is no automatic
//! mixing of formats and no migration between them.
//!
//! ⚠️ There are no bytes of the template here and there can be no: the registry record
//! contains only a link from the provider (Document 29, D-03).

use core::fmt;

use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId};
use prost::Message;

use crate::limits;
use crate::model::StorageError;
use crate::redb::codec::{ENVELOPE_MAGIC, checksum};

/// The version of the diagram that we write and read.
mod schema;
pub use schema::SCHEMA_VERSION;

// Several oneof values are a corrupted record, even if the checksum is correct. Once parsed,
// the prost would retain only the last value.

pub mod proto;

#[path = "enrollment_codec.tests.rs"]
#[cfg(test)]
mod tests;

mod enrollment_record;
pub use enrollment_record::EnrollmentRecord;
mod enrollment_status;
pub use enrollment_status::EnrollmentStatus;
mod record_error;
pub use record_error::RecordError;

mod record_codec;
pub use record_codec::decode;
pub use record_codec::decode_v3;
pub use record_codec::encode;
pub use record_codec::encode_v3;
mod envelope_codec;
use envelope_codec::envelope;
pub(crate) use envelope_codec::envelope_with;
use envelope_codec::unenvelope;
pub(crate) use envelope_codec::unenvelope_with;
