use carosign_runtime::ports::CallerContext;

use super::PreparedOwnerVerification;

#[allow(dead_code)]
pub(super) struct RetainedOwnerVerification {
    pub(super) prepared: PreparedOwnerVerification,
    pub(super) caller: CallerContext,
    pub(super) cursor: u64,
}
