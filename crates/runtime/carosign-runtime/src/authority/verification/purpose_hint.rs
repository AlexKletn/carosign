/// Customer tip; the final goal is determined by policy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PurposeHint {
    /// Entrance.
    SessionLogin,
    /// Unblocking.
    SessionUnlock,
    /// Preferential action.
    PrivilegedAction,
    /// Self-testing.
    SelfCheck,
    KeyOperation,
}
