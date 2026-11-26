use crate::ports::CallerContext;
use core::fmt;

/// Owner of the operation.
#[derive(Clone, PartialEq, Eq)]
pub struct Owner {
    /// Who created it.
    pub caller: CallerContext,
}

impl fmt::Debug for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Owner(..)")
    }
}
