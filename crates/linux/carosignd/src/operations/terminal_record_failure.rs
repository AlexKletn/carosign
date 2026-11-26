/// Refusal to execute the inspection terminal record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalRecordFailure {
    OperationUnavailable,
    ContextMismatch,
    AwaitingActor,
    AlreadyRecorded,
    MailboxFull,
}
