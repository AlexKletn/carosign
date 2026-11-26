//! Metadata Storage: Interface, Transactions, and Sustainability Report.
//!
//! ⚠️ The rule of thumb (document 10): **Stable recording is a report**, not no error. If the
//! back can't confirm that the data has been sent to the disk, no altering operation is sent.

use core::fmt;

use crate::model::StorageError;

mod table_data;
pub use table_data::{Entry, TableMap};

pub mod tests_support;

mod durable_receipt;
pub use durable_receipt::DurableReceipt;
mod read_transaction;
pub use read_transaction::ReadTransaction;
mod write_transaction;
pub use write_transaction::WriteTransaction;
mod table;
pub use table::Table;
mod metadata_store;
pub use metadata_store::MetadataStore;
