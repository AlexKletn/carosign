use super::ProcessState;

/// What to do after the death of an worker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AfterDeath {
    /// Terminate access by completion; the device is a separate issue.
    AccessRevokedByKernel,
    /// Access could remain: complete and verify the reset.
    AccessMayRemain,
}

/// A decision on death.
///
/// ⚠️ The "kernel has stopped access" sign is evidence, not guesswork. `AccessMayRemain`And
/// then the operation goes into reconciliation.
pub fn after_death(state: ProcessState, kernel_enforced: bool) -> AfterDeath {
    match state {
        ProcessState::Running => AfterDeath::AccessMayRemain,
        ProcessState::Exited { .. } | ProcessState::Signalled => {
            if kernel_enforced {
                AfterDeath::AccessRevokedByKernel
            } else {
                AfterDeath::AccessMayRemain
            }
        }
        ProcessState::Unknown => AfterDeath::AccessMayRemain,
    }
}
