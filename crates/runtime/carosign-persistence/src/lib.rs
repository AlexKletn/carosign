//! Metadata Storage: Registry, Change Log, and Submission queue in a single redb transaction
//! area, plus limited template storage.
//!
//! The device -- `docs/specs/engineering/implementation-v06/10-persistence-and-recovery.md`.

pub mod bindings;
pub mod enrollment_codec;
pub mod idempotency;
#[cfg(test)]
mod installation_ledger;
pub mod installation_records;
pub mod integrity;
pub mod journal;
pub mod journal_store;
pub mod limits;
pub mod migration;
pub mod model;
pub mod mutation_plan;
pub mod mutation_records;
pub mod mutation_store;
pub mod outbox;
pub mod projection;
pub mod recovery;
pub mod redb;
pub mod registry;
pub mod store;
pub mod template;
pub mod transaction;

/// Stamp vault for inspections.
///
/// ⚠️ Not for use in the work: stability is here **stated **, not tested.
#[doc(hidden)]
pub mod test_support {
    pub use crate::store::tests_support::*;
}
