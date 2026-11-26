/// The stage of one sent inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationPhase {
    /// Command's out, no confirmation yet.
    AwaitingAck,
    /// Command accepted by the worker.
    Running,
    /// Received the result or refusal of the command.
    Finished,
    /// Violation or cliff.
    Failed,
}
