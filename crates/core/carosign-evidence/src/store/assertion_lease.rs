use super::ProviderAssertionRecord;
use super::SlotContents;
use super::SlotError;
use super::SlotState;
use crate::material::EvidenceMaterialRef;
use carosign_core::operation::context::BootInstant;
use std::sync::Arc;
use std::sync::Mutex;

/// One-time possession of the statement record without byte material.
pub struct AssertionLease {
    pub(super) inner: Arc<Mutex<SlotContents>>,
    pub(super) record: ProviderAssertionRecord,
    pub(super) reference: EvidenceMaterialRef,
}

impl AssertionLease {
    /// A link issued by the record holder for this one-time issue.
    pub fn reference(&self) -> &EvidenceMaterialRef {
        &self.reference
    }
    /// The scope of approval is available only to the owner of a one-time issue.
    pub fn record(&self) -> &ProviderAssertionRecord {
        &self.record
    }
    /// Checks the deadline and recall before applying the result of the inspection.
    pub fn still_valid(&self, now: BootInstant) -> Result<(), SlotError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.deadline.is_expired_at(now) {
            if matches!(inner.state, SlotState::Checking) {
                inner.state = SlotState::RevokedInUse;
            }
            return Err(SlotError::Expired);
        }
        if !matches!(inner.state, SlotState::Checking) {
            return Err(SlotError::Revoked);
        }
        Ok(())
    }

    /// Completes a one-time issue with a check of the deadline and recall.
    pub fn finish(self, now: BootInstant) -> Result<(), SlotError> {
        self.still_valid(now)
    }
}

impl Drop for AssertionLease {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.state = SlotState::Destroyed;
    }
}
