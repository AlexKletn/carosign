//! Implementing the journal on top of [`MetadataStore`](crate::store::MetadataStore).
//!
//! ⚠️ All phases of the change occur **here*, not in runtime: runtime only gets the ticket
//! and the total.

#[cfg(all(feature = "legacy-fixtures", not(debug_assertions)))]
compile_error!("legacy-fixtures предназначен только для отладочных проверок");

mod current_scope;
mod dispatch_outbox;
mod read_mutation;
mod read_schema;
mod record_lock;
mod store_journal;

use current_scope::check_current_scope;
pub(crate) use current_scope::{
    check_current_scope_v3, read_current_scope_v3, read_indexed_scope_v3,
};
use dispatch_outbox::check_dispatch_outbox;
use read_mutation::read_mutation;
use read_schema::{ReadSchema, read_schema, require_legacy_schema};
use record_lock::check_record_lock;
pub use store_journal::StoreJournal;
