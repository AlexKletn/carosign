/// Closing a channel without a message does not in itself prove a successful exec: the
/// process could also have been completed before it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ExecReport {
    Failure { stage: u8, errno: i32 },
    ClosedWithoutReport,
}
