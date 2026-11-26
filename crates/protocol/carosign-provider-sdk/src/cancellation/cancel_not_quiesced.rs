use super::CancelState;
use core::fmt;

/// The reason the cancellation has not been confirmed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CancelNotQuiesced {
    /// Where we're staying.
    pub state: CancelState,
}

impl fmt::Display for CancelNotQuiesced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "отмена не подтверждена: состояние {:?}", self.state)
    }
}

impl core::error::Error for CancelNotQuiesced {}
