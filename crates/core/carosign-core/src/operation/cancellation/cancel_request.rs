use super::CancelSource;
use core::fmt;

/// Please cancel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CancelRequest {
    /// Who asks?
    pub source: CancelSource,
}

impl fmt::Debug for CancelRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancelRequest")
            .field("source", &self.source)
            .finish()
    }
}
