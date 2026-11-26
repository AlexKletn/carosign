use super::AssertionLease;
use super::EvidenceLease;
use super::ProviderAssertionRecord;
use super::SlotContents;
use super::SlotError;
use super::SlotState;
use crate::material::EvidenceMaterialRef;
use crate::material::OwnerIdentity;
use carosign_core::ids::EvidenceId;
use carosign_core::ids::OperationId;
use carosign_core::operation::context::BootDeadline;
use carosign_core::operation::context::BootInstant;
use sha2::Digest;
use sha2::Sha256;
use std::sync::Arc;
use std::sync::Mutex;
use zeroize::Zeroizing;

// Document 20 restricts the opaque 16KB token; the owner is independent of the wire crate.
const OPAQUE_MATERIAL_MAX: usize = 16 * 1024;

/// A single slot is limited to the size of the token from the document 20.
pub struct EvidenceSlot {
    pub(super) inner: Arc<Mutex<SlotContents>>,
}

impl EvidenceSlot {
    /// The ID comes from the system source, the deadline comes from the verified operation.
    pub fn new(id: EvidenceId, operation: OperationId, deadline: BootDeadline) -> Self {
        Self::new_scoped(id, operation, deadline, Arc::new(OwnerIdentity), 1)
    }

    pub(super) fn new_scoped(
        id: EvidenceId,
        operation: OperationId,
        deadline: BootDeadline,
        owner: Arc<OwnerIdentity>,
        generation: u64,
    ) -> Self {
        let reference =
            EvidenceMaterialRef::issued(*id.as_bytes(), *operation.as_bytes(), owner, generation)
                .expect("типизированные ID не равны нулю");
        Self {
            inner: Arc::new(Mutex::new(SlotContents {
                reference,
                deadline,
                state: SlotState::Empty,
            })),
        }
    }

    /// Returns the link only after accepting the bytes; re-issuance is prohibited.
    pub fn hold(
        &self,
        bytes: Zeroizing<Vec<u8>>,
        now: BootInstant,
    ) -> Result<EvidenceMaterialRef, SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(inner.state, SlotState::Empty)
            || bytes.is_empty()
            || bytes.len() > OPAQUE_MATERIAL_MAX
        {
            return Err(SlotError::Unavailable);
        }
        if inner.deadline.is_expired_at(now) {
            inner.state = SlotState::NeverIssued;
            return Err(SlotError::Expired);
        }
        inner.reference.bind_digest(Sha256::digest(&*bytes).into());
        inner.state = SlotState::Held(bytes);
        Ok(inner.reference.clone())
    }

    pub(super) fn hold_assertion(
        &self,
        record: ProviderAssertionRecord,
        now: BootInstant,
    ) -> Result<EvidenceMaterialRef, SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(inner.state, SlotState::Empty) {
            return Err(SlotError::Unavailable);
        }
        if inner.deadline.is_expired_at(now) {
            inner.state = SlotState::NeverIssued;
            return Err(SlotError::Expired);
        }
        inner.state = SlotState::HeldAssertion(Box::new(record));
        Ok(inner.reference.clone())
    }

    pub(super) fn take_assertion(
        &self,
        reference: EvidenceMaterialRef,
        now: BootInstant,
    ) -> Result<AssertionLease, SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if reference != inner.reference {
            return Err(SlotError::WrongReference);
        }
        if inner.deadline.is_expired_at(now) {
            if matches!(inner.state, SlotState::HeldAssertion(_)) {
                inner.state = SlotState::Destroyed;
            }
            return Err(SlotError::Expired);
        }
        let record = match std::mem::replace(&mut inner.state, SlotState::Checking) {
            SlotState::HeldAssertion(record) => *record,
            state => {
                inner.state = state;
                return Err(SlotError::Unavailable);
            }
        };
        Ok(AssertionLease {
            inner: Arc::clone(&self.inner),
            record,
            reference,
        })
    }

    /// One-time transfers the retained bytes to the inspector.
    pub fn take(
        &self,
        reference: EvidenceMaterialRef,
        now: BootInstant,
    ) -> Result<EvidenceLease, SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if reference != inner.reference {
            return Err(SlotError::WrongReference);
        }
        if inner.deadline.is_expired_at(now) {
            if matches!(inner.state, SlotState::Empty) {
                inner.state = SlotState::NeverIssued;
            } else if matches!(inner.state, SlotState::Held(_)) {
                inner.state = SlotState::Destroyed;
            } else if matches!(inner.state, SlotState::Checking) {
                inner.state = SlotState::RevokedInUse;
            }
            return Err(SlotError::Expired);
        }
        match std::mem::replace(&mut inner.state, SlotState::Checking) {
            SlotState::Held(bytes) => Ok(EvidenceLease {
                inner: Arc::clone(&self.inner),
                bytes,
            }),
            state => {
                inner.state = state;
                Err(SlotError::Unavailable)
            }
        }
    }

    /// Success means that bytes are no longer held by either the slot or the output.
    pub fn invalidate(&self, reference: EvidenceMaterialRef) -> Result<(), SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if reference != inner.reference {
            return Err(SlotError::WrongReference);
        }
        match inner.state {
            SlotState::Held(_) | SlotState::HeldAssertion(_) | SlotState::Destroyed => {
                inner.state = SlotState::Destroyed;
                Ok(())
            }
            SlotState::Checking | SlotState::RevokedInUse => {
                inner.state = SlotState::RevokedInUse;
                Err(SlotError::InUse)
            }
            SlotState::Empty | SlotState::NeverIssued => Err(SlotError::Unavailable),
        }
    }

    /// The term observer causes this regardless of the examiner.
    pub fn expire(&self, now: BootInstant) -> Result<(), SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !inner.deadline.is_expired_at(now) {
            return Err(SlotError::Unavailable);
        }
        // Time checks and recalls are one transition, otherwise the checker would have had
        // time to get the material with the old clock reading between them.
        match inner.state {
            SlotState::Empty => {
                inner.state = SlotState::NeverIssued;
                Err(SlotError::Unavailable)
            }
            SlotState::Held(_) | SlotState::HeldAssertion(_) | SlotState::Destroyed => {
                inner.state = SlotState::Destroyed;
                Ok(())
            }
            SlotState::Checking | SlotState::RevokedInUse => {
                inner.state = SlotState::RevokedInUse;
                Err(SlotError::InUse)
            }
            SlotState::NeverIssued => Err(SlotError::Unavailable),
        }
    }
}

impl Drop for EvidenceSlot {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.state = match inner.state {
            SlotState::Checking | SlotState::RevokedInUse => SlotState::RevokedInUse,
            _ => SlotState::Destroyed,
        };
    }
}
