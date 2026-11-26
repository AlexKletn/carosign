use std::io;

use super::{AdmissionError, GateSender, admission_state::AdmissionState};

/// A single point in the order of admission and accepted recall of one launch.
///
/// Generations and trust in the source must be verified by the system owner; this machine
/// itself does not certify the installation or policy. Successful transmission through the
/// gate means only the transfer of permission, not a successful exec.
pub(crate) struct AdmissionGate {
    inner: std::sync::Mutex<AdmissionState>,
}

impl AdmissionGate {
    pub(crate) fn new(generation: u64, sender: GateSender) -> io::Result<Self> {
        if generation == 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(Self {
            inner: std::sync::Mutex::new(AdmissionState::Pending { generation, sender }),
        })
    }

    /// Revocation accepted after receipt of the lock; the already transferred gate is not
    /// withdrawn.
    pub(crate) fn revoke(&self) -> Result<(), AdmissionError> {
        let mut state = self.inner.lock().map_err(|_| AdmissionError::Closed)?;
        match &*state {
            AdmissionState::Pending { .. } => {
                *state = AdmissionState::Revoked;
                Ok(())
            }
            _ => Err(AdmissionError::Closed),
        }
    }

    /// Generation Verification and one-time broadcast are serialized with a review.
    pub(crate) fn admit(&self, generation: u64) -> Result<(), AdmissionError> {
        let mut state = self.inner.lock().map_err(|_| AdmissionError::Closed)?;
        match &*state {
            AdmissionState::Pending {
                generation: expected,
                ..
            } if *expected != generation => {
                return Err(AdmissionError::StaleGeneration);
            }
            AdmissionState::Pending { .. } => {}
            _ => return Err(AdmissionError::Closed),
        }
        // Once you start sending, the error does not allow you to re-issue the gate.
        let AdmissionState::Pending { sender, .. } =
            std::mem::replace(&mut *state, AdmissionState::Failed)
        else {
            unreachable!();
        };
        sender.release().map_err(|_| AdmissionError::Io)?;
        *state = AdmissionState::Released;
        Ok(())
    }
}

#[cfg(test)]
#[path = "admission_gate.tests.rs"]
mod tests;
