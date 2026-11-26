use super::{BlockedMutationScope, StartupScan};

/// Solutions and areas tested in a single storage image.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StartupScopeScan {
    /// Journal decisions.
    pub scan: StartupScan,
    /// All unfinished areas, including those waiting to be cleaned.
    pub blocked: Vec<BlockedMutationScope>,
}
