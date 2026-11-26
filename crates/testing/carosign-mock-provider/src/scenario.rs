#[derive(Clone, Copy)]
pub(super) enum Scenario {
    NoSample,
    Matched,
    MatchedScope,
    SeededMatchedScope,
    MatchedToken,
    SeededNoSample,
    SeededMatchedToken,
    PendingCancel,
    SeededPendingCancel,
    LinuxNoSample,
    LinuxMatchedScope,
    LinuxPendingCancel,
    LinuxPersistent,
    LinuxPendingMutation,
    LinuxPendingRollback,
    LinuxErrorRollback,
    LinuxLostRollback,
    LinuxCommittedRollback,
    #[cfg(feature = "ssh-mock")]
    SshMock,
}
