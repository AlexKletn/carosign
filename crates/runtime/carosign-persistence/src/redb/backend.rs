//! redb as the back of the storage: tables, reading, writing and honest stability.
//!
//! ⚠️ The rule for which this file exists (Document 10): `commit` It's a report, not an
//! absence of error. `commit_durable` It takes the record to the disc and then it says,
//! "Ready," and the buffered record is only passed off as a success by someone who doesn't
//! check it.
//!
//! ⚠️ The values lie ** in the shell of the scheme** from [`crate::redb::codec`]: magic,
//! length, load, sum. Internal layout `Vec<u8>` It is not read as a record directly, only
//! through analysis.
//!
//! ⚠️ Transaction doesn't cross `await` and does not live longer than one step:
//! `WriteTx`/`ReadTx` hold `redb`-object and not. `Send`.

mod backup;
mod fence_store;
mod preflight;
mod quarantine_recovery;
mod transactions;

#[cfg(test)]
pub(crate) use backup::CreatedV3Backup;
pub(crate) use backup::VerifiedV3Backup;
pub use fence_store::{DurableReleasePermit, FenceInventory, FenceScope, InitialFenceStore};
use preflight::PreflightBackend;
pub use quarantine_recovery::QuarantineRecoveryStore;
use transactions::WriteTx;
pub(crate) use transactions::{GuardedWriteTx, ReadTx, ReadTxRef};

use std::fmt;
use std::path::Path;

use redb::{
    Database, MultimapTableHandle, ReadableTable, ReadableTableMetadata, StorageBackend,
    TableDefinition, TableHandle,
};

use crate::model::StorageError;
use crate::store::{
    DurableReceipt, Entry, MetadataStore, ReadTransaction, Table, WriteTransaction,
};

/// Tables of the v1 scheme as redb sees them.
///
/// ⚠️ Keys and values are raw bytes: meaning is given by the codec, not by the type of table.
/// `redb` Keeps the dimensions.
const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta");
const AUTHENTICATORS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("authenticators");
const ENROLLMENTS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("enrollments");
const ENROLLMENT_INDEX: TableDefinition<&[u8], &[u8]> = TableDefinition::new("enrollment_index");
const MUTATIONS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("mutations");
const SCOPE_LOCK: TableDefinition<&[u8], &[u8]> = TableDefinition::new("mutation_scope_lock");
const OUTBOX: TableDefinition<&[u8], &[u8]> = TableDefinition::new("effect_outbox");
const IDEMPOTENCY: TableDefinition<&[u8], &[u8]> = TableDefinition::new("idempotency");
const PROVIDER_STATE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("provider_state");
const PACKAGES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("packages");
const TEMPLATE_BLOBS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("template_blobs");

pub(crate) const SCHEMA_KEY: &[u8] = b"biometric.schema";
pub(crate) const SCHEMA_V3: &[u8] = b"BSC1\0\0\0\x03\0\0\0\0";
const SCHEMA_V4: &[u8] = b"BSC1\0\0\0\x04\0\0\0\0";
const SCHEMA_V5: &[u8] = b"BSC1\0\0\0\x05\0\0\0\0";
const RESOURCE_FENCES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("resource_fences");
const RESOURCE_FENCE_VALUE: &[u8] = b"RF01";
const SCOPED_FENCE_MAGIC: &[u8; 4] = b"RF02";
const INSTALLATIONS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("provider_installations");

fn definition(table: Table) -> TableDefinition<'static, &'static [u8], &'static [u8]> {
    match table {
        Table::Meta => META,
        Table::Authenticators => AUTHENTICATORS,
        Table::Enrollments => ENROLLMENTS,
        Table::EnrollmentIndex => ENROLLMENT_INDEX,
        Table::Mutations => MUTATIONS,
        Table::MutationScopeLock => SCOPE_LOCK,
        Table::EffectOutbox => OUTBOX,
        Table::Idempotency => IDEMPOTENCY,
        Table::ProviderState => PROVIDER_STATE,
        Table::Packages => PACKAGES,
        Table::TemplateBlobs => TEMPLATE_BLOBS,
    }
}

#[path = "backend.tests.rs"]
#[cfg(test)]
mod preflight_tests;

mod redb_store;
pub use redb_store::RedbStore;
mod shared_backend;
use shared_backend::SharedBackend;
