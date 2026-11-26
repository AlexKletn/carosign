use super::{GateSender, PolicyAdmissionError, policy_admission_state::PolicyAdmissionState};

/// Arranges the admissions of one policy holder and the feedback he accepts.
///
/// The source of the policy and the right to invoke these methods are not verified here.
/// Until the system channel is connected, this owner is not authorized.
pub(crate) struct PolicyAdmissions {
    state: std::sync::Mutex<PolicyAdmissionState>,
}

const MAX_POLICY_ADMISSIONS: usize = 64;

impl PolicyAdmissions {
    pub(crate) fn new(generation: u64) -> Result<Self, PolicyAdmissionError> {
        if generation == 0 {
            return Err(PolicyAdmissionError::StaleGeneration);
        }
        Ok(Self {
            state: std::sync::Mutex::new(PolicyAdmissionState {
                generation,
                revoked: false,
                next: 1,
                pending: std::collections::BTreeMap::new(),
            }),
        })
    }

    pub(crate) fn register(
        &self,
        generation: u64,
        sender: GateSender,
    ) -> Result<u64, PolicyAdmissionError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        if state.revoked {
            return Err(PolicyAdmissionError::Closed);
        }
        if generation != state.generation {
            return Err(PolicyAdmissionError::StaleGeneration);
        }
        if state.pending.len() >= MAX_POLICY_ADMISSIONS {
            return Err(PolicyAdmissionError::Limit);
        }
        let id = state.next;
        state.next = id.checked_add(1).ok_or(PolicyAdmissionError::Limit)?;
        state.pending.insert(id, sender);
        Ok(id)
    }

    /// Taking admission and sending one byte is serialized with a recall.
    pub(crate) fn admit(&self, generation: u64, id: u64) -> Result<(), PolicyAdmissionError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        if state.revoked {
            return Err(PolicyAdmissionError::Closed);
        }
        if generation != state.generation {
            return Err(PolicyAdmissionError::StaleGeneration);
        }
        let sender = state
            .pending
            .remove(&id)
            .ok_or(PolicyAdmissionError::Unknown)?;
        // Even a sending error does not return a disposable gate to the table.
        sender.release().map_err(|_| PolicyAdmissionError::Io)
    }

    /// After the accepted recall, all waiting assistants receive a gate closure.
    pub(crate) fn revoke(&self) -> Result<(), PolicyAdmissionError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PolicyAdmissionError::Closed)?;
        if state.revoked {
            return Err(PolicyAdmissionError::Closed);
        }
        state.revoked = true;
        state.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
#[path = "policy_admissions.tests.rs"]
mod tests;
