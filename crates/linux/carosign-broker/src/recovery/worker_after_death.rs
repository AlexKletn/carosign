/// Assessment after the death of the worker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkerAfterDeath {
    /// Access is terminated when the process is complete; the device is a separate issue.
    AccessRevoked,
    /// Access could remain: you need to complete the process and check the reset.
    AccessMayRemain,
}

/// Can access be terminated by completing the process?
///
/// ⚠️ The answer “yes” means “device is safe”: it is a separate driver or reset evidence.
pub fn revoke_by_termination(supported: bool) -> WorkerAfterDeath {
    if supported {
        WorkerAfterDeath::AccessRevoked
    } else {
        WorkerAfterDeath::AccessMayRemain
    }
}
