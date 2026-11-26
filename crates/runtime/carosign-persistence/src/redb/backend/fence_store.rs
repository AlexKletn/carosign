use crate::store::WriteTransaction;
use redb::{ReadableTable, ReadableTableMetadata, TableHandle};

use super::{
    AUTHENTICATORS, Database, DurableReceipt, ENROLLMENT_INDEX, ENROLLMENTS, GuardedWriteTx,
    IDEMPOTENCY, INSTALLATIONS, META, MUTATIONS, OUTBOX, PACKAGES, PROVIDER_STATE, Path,
    PreflightBackend, RESOURCE_FENCE_VALUE, RESOURCE_FENCES, ReadTxRef, SCHEMA_KEY, SCHEMA_V5,
    SCOPE_LOCK, SCOPED_FENCE_MAGIC, SharedBackend, StorageError, TEMPLATE_BLOBS, WriteTx,
};

fn scoped_fence_resources(value: &[u8]) -> Option<Vec<[u8; 16]>> {
    const BASE: usize = 77;
    if value.len() < BASE || &value[..4] != SCOPED_FENCE_MAGIC {
        return None;
    }
    for chunk in value[4..68].chunks_exact(16) {
        if chunk.iter().all(|&byte| byte == 0) {
            return None;
        }
    }
    if u64::from_le_bytes(value[68..76].try_into().ok()?) == 0 {
        return None;
    }
    let count = value[76] as usize;
    if count == 0
        || count > carosign_core::resource::RESOURCE_SET_MAX
        || value.len() != BASE + count * 17
    {
        return None;
    }
    let mut ids = Vec::with_capacity(count);
    for claim in value[BASE..].chunks_exact(17) {
        let id: [u8; 16] = claim[..16].try_into().ok()?;
        if id == [0; 16]
            || !matches!(claim[16], 1..=3)
            || ids.last().is_some_and(|previous| *previous >= id)
        {
            return None;
        }
        ids.push(id);
    }
    Some(ids)
}

mod initial_fence_store;
pub use initial_fence_store::InitialFenceStore;
mod fence_scope;
pub use fence_scope::FenceScope;
mod durable_release_permit;
pub use durable_release_permit::DurableReleasePermit;

mod fence_inventory;
pub use fence_inventory::FenceInventory;
