use carosign_runtime::operation::actor::Step;

use crate::operations::TerminalRecordFailure;

/// The result of the recording is retained until the transfer to the owner of the
/// performance.
pub(crate) enum RecordResult {
    Completed(Result<Step, TerminalRecordFailure>),
}
