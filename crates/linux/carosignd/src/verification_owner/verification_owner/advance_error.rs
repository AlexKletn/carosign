use super::{DriveError, OwnerError};

/// Failure to receive or process; saved steps are not canceled.
#[derive(Debug)]
pub(crate) enum AdvanceError {
    Owner(OwnerError),
    Drive(DriveError),
}
