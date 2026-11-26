use super::{ConnectionHandle, ConnectionUsage, Entry};
use carosign_core::ids::{IdempotencyKey, OperationId};
use std::collections::BTreeMap;

/// The table is not a right of access to the operation.
#[derive(Default)]
pub(super) struct RegistryEntries {
    pub(super) operations: BTreeMap<OperationId, Entry>,
    pub(super) connections: BTreeMap<ConnectionHandle, ConnectionUsage>,
    pub(super) retired: BTreeMap<OperationId, super::retired_operation::RetiredOperation>,
    pub(super) next_connection: u128,
    pub(super) idempotency: BTreeMap<
        ([u8; 32], IdempotencyKey),
        (crate::verification_idempotency::RequestDigest, OperationId),
    >,
}

impl std::ops::Deref for RegistryEntries {
    type Target = BTreeMap<OperationId, Entry>;
    fn deref(&self) -> &Self::Target {
        &self.operations
    }
}
impl std::ops::DerefMut for RegistryEntries {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.operations
    }
}
