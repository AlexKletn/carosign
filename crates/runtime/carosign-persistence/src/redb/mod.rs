//! Back of the vault.
//!
//! ⚠️ The selection of a particular library (redb) is deferred: first, the **schema** -
//! tables, keys, values and rules (documents 10, 23) is
//! described.`MetadataStore`](crate::store::MetadataStore): open the reading, open the
//! recording and **confirm the disc when fixing.

pub mod backend;
pub mod codec;
pub mod record;
pub mod snapshot;
pub mod tables;
pub mod tx;
