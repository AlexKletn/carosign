use super::Priority;
use core::fmt;

/// The mail's full.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MailboxFull {
    /// What a turn.
    pub priority: Priority,
}

impl fmt::Debug for MailboxFull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MailboxFull({:?})", self.priority)
    }
}

impl fmt::Display for MailboxFull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "очередь {:?} переполнена", self.priority)
    }
}

impl core::error::Error for MailboxFull {}
