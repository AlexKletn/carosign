use super::SlotContents;
use super::SlotError;
use super::SlotState;
use carosign_core::operation::context::BootInstant;
use std::sync::Arc;
use std::sync::Mutex;
use zeroize::Zeroizing;

/// The material is in the test room. When the holder is lost, the bytes are cleared.
pub struct EvidenceLease {
    pub(super) inner: Arc<Mutex<SlotContents>>,
    pub(super) bytes: Zeroizing<Vec<u8>>,
}

impl EvidenceLease {
    /// Available only to the inspector who owns a one-time issue.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Verification before using the result does not prove the origin.
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

    /// Completes a one-time atomic review check and clears bytes.
    /// Issuance of an assessment does not replace re-checking of the delivery policy.
    pub fn finish(mut self, now: BootInstant) -> Result<(), SlotError> {
        use zeroize::Zeroize;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let result = if inner.deadline.is_expired_at(now) {
                Err(SlotError::Expired)
            } else if !matches!(inner.state, SlotState::Checking) {
                Err(SlotError::Revoked)
            } else {
                Ok(())
            };
            self.bytes.zeroize();
            inner.state = SlotState::Destroyed;
            result
        }
    }
}

impl Drop for EvidenceLease {
    fn drop(&mut self) {
        // First we wipe the bytes, then we make available the confirmation of the review.
        use zeroize::Zeroize;
        self.bytes.zeroize();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.state = SlotState::Destroyed;
    }
}
