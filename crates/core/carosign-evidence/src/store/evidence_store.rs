use super::AssertionLease;
use super::EvidenceLease;
use super::EvidenceSlot;
use super::ProviderAssertionRecord;
use super::SlotError;
use super::SlotState;
use super::StoreInner;
use crate::material::EvidenceMaterialRef;
use crate::material::OwnerIdentity;
use carosign_core::ids::EvidenceId;
use carosign_core::ids::IdSource;
use carosign_core::ids::OperationId;
use carosign_core::operation::context::BootDeadline;
use carosign_core::operation::context::BootInstant;
use std::sync::Arc;
use std::sync::Mutex;
use zeroize::Zeroizing;

/// Simultaneous limit of materials of one owner.
const STORE_MAX: usize = 256;

/// Limited possession of current service material.
///
/// The place is vacated after cleaning and confirming the transfer of the actor's step.
/// The identity of the owner and the generation prevents the old reference from referring to
/// the next material; ownership does not establish its origin.
pub struct EvidenceStore {
    limit: usize,
    owner: Arc<OwnerIdentity>,
    inner: Mutex<StoreInner>,
}

impl EvidenceStore {
    /// The presence of the record does not confirm the possession of the material or its
    /// destruction.
    /// Verification is only suitable as a preliminary prohibition of the release of the
    /// operation.
    pub fn contains_operation(&self, operation: OperationId) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .contains_key(&operation)
    }

    /// The limit conveys the composition; it does not exceed the existing limit of
    /// operations.
    pub fn new(limit: usize) -> Option<Self> {
        (limit > 0 && limit <= STORE_MAX).then(|| Self {
            limit,
            owner: Arc::new(OwnerIdentity),
            inner: Mutex::new(StoreInner {
                entries: std::collections::BTreeMap::new(),
                generation: 0,
            }),
        })
    }

    /// Issues a link only after accepting bytes and fixing the number.
    pub fn hold(
        &self,
        id: EvidenceId,
        operation: OperationId,
        deadline: BootDeadline,
        bytes: Zeroizing<Vec<u8>>,
        now: BootInstant,
    ) -> Result<EvidenceMaterialRef, SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let entries = &inner.entries;
        if entries.len() >= self.limit
            || entries.contains_key(&operation)
            || entries.values().any(|(existing, _)| *existing == id)
        {
            return Err(SlotError::Unavailable);
        }
        let generation = inner
            .generation
            .checked_add(1)
            .ok_or(SlotError::GenerationExhausted)?;
        let slot =
            EvidenceSlot::new_scoped(id, operation, deadline, self.owner.clone(), generation);
        let reference = slot.hold(bytes, now)?;
        inner.generation = generation;
        inner.entries.insert(operation, (id, slot));
        Ok(reference)
    }

    /// Reserves platform ID for approval without fictitious bytes.
    /// The origin of the session and the full context are verified by the caller.
    pub fn hold_assertion(
        &self,
        ids: &dyn IdSource,
        record: ProviderAssertionRecord,
        now: BootInstant,
    ) -> Result<EvidenceMaterialRef, SlotError> {
        if !record.valid() {
            return Err(SlotError::Unavailable);
        }
        let id = ids.new_evidence_id().map_err(|_| SlotError::Unavailable)?;
        let operation = record.operation;
        let deadline = record.deadline;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.entries.len() >= self.limit
            || inner.entries.contains_key(&operation)
            || inner.entries.values().any(|(existing, _)| *existing == id)
        {
            return Err(SlotError::Unavailable);
        }
        let generation = inner
            .generation
            .checked_add(1)
            .ok_or(SlotError::GenerationExhausted)?;
        let slot =
            EvidenceSlot::new_scoped(id, operation, deadline, self.owner.clone(), generation);
        let reference = slot.hold_assertion(record, now)?;
        inner.generation = generation;
        inner.entries.insert(operation, (id, slot));
        Ok(reference)
    }

    /// Once transmits a record of the approval to the inspector.
    pub fn take_assertion(
        &self,
        reference: EvidenceMaterialRef,
        now: BootInstant,
    ) -> Result<AssertionLease, SlotError> {
        let operation = OperationId::from_bytes(reference.operation())
            .map_err(|_| SlotError::WrongReference)?;
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (_, slot) = inner
            .entries
            .get(&operation)
            .ok_or(SlotError::WrongReference)?;
        slot.take_assertion(reference, now)
    }

    /// Transmits the material once; does not accept someone else's link.
    pub fn take(
        &self,
        reference: EvidenceMaterialRef,
        now: BootInstant,
    ) -> Result<EvidenceLease, SlotError> {
        let operation = OperationId::from_bytes(reference.operation())
            .map_err(|_| SlotError::WrongReference)?;
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (_, slot) = inner
            .entries
            .get(&operation)
            .ok_or(SlotError::WrongReference)?;
        slot.take(reference, now)
    }

    /// Confirmation is possible only after cleaning the relevant issue.
    pub fn invalidate(&self, reference: EvidenceMaterialRef) -> Result<(), SlotError> {
        let operation = OperationId::from_bytes(reference.operation())
            .map_err(|_| SlotError::WrongReference)?;
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (_, slot) = inner
            .entries
            .get(&operation)
            .ok_or(SlotError::WrongReference)?;
        slot.invalidate(reference)
    }

    /// The control signal recalls the material even when the actor is busy.
    pub fn invalidate_operation(&self, operation: OperationId) -> Result<(), SlotError> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (_, slot) = inner
            .entries
            .get(&operation)
            .ok_or(SlotError::WrongReference)?;
        let reference = slot
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reference
            .clone();
        slot.invalidate(reference)
    }

    /// Returns the current link to the already accepted material by verifying both IDs.
    pub fn reference_for(
        &self,
        operation: OperationId,
        id: EvidenceId,
    ) -> Result<EvidenceMaterialRef, SlotError> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (stored, slot) = inner
            .entries
            .get(&operation)
            .ok_or(SlotError::WrongReference)?;
        if *stored != id {
            return Err(SlotError::WrongReference);
        }
        let reference = slot
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reference
            .clone();
        Ok(reference)
    }

    /// Makes room after the actor and performer take the step of destruction.
    /// The caller is obliged to confirm the transmission of the step; one clearing of the
    /// bytes is not such confirmation.
    pub fn release_after_handoff(&self, reference: &EvidenceMaterialRef) -> Result<(), SlotError> {
        let operation = OperationId::from_bytes(reference.operation())
            .map_err(|_| SlotError::WrongReference)?;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (_, slot) = inner
            .entries
            .get(&operation)
            .ok_or(SlotError::WrongReference)?;
        {
            let slot_inner = slot.inner.lock().unwrap_or_else(|e| e.into_inner());
            if &slot_inner.reference != reference {
                return Err(SlotError::WrongReference);
            }
            if !matches!(slot_inner.state, SlotState::Destroyed) {
                return Err(SlotError::InUse);
            }
        }
        inner.entries.remove(&operation);
        Ok(())
    }

    /// Withdraws all overdue issuances; the busy issuance continues the withdrawal.
    pub fn expire(&self, now: BootInstant) {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        for (_, slot) in inner.entries.values() {
            let _ = slot.expire(now);
        }
    }
}

#[path = "evidence_store.tests.rs"]
#[cfg(test)]
mod reuse_tests;
