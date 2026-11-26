use super::ProcessState;

/// The expectation of death of the trial is limited.
///
/// ⚠️ The endless waiting for death is the hanging of service. `Unknown`, not "probably
/// dead."
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WaitBudget {
    /// Term, Ms.
    pub timeout_ms: u64,
}

impl WaitBudget {
    /// Normal cleaning time (document 13).
    pub fn cleanup() -> Self {
        Self {
            timeout_ms: u64::from(carosign_core::error::limits::CLEANUP_TIMEOUT_S) * 1_000,
        }
    }

    /// The result of waiting.
    pub fn outcome(self, observed: Option<ProcessState>) -> ProcessState {
        observed.unwrap_or(ProcessState::Unknown)
    }
}
