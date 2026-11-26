use super::OwnerChannel;
use crate::ports::CallerContext;
use core::fmt;

/// Owner of the operation.
#[derive(Clone, PartialEq, Eq)]
pub struct OwnerHandle {
    caller: CallerContext,
    channel: OwnerChannel,
}

impl fmt::Debug for OwnerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnerHandle")
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

impl OwnerHandle {
    /// Owner by the context of the caller.
    pub fn new(caller: CallerContext) -> Self {
        Self {
            caller,
            channel: OwnerChannel::Attached,
        }
    }

    /// Context of the caller (to re-check the authority).
    pub fn caller(&self) -> &CallerContext {
        &self.caller
    }

    /// Communication.
    pub fn channel(&self) -> OwnerChannel {
        self.channel
    }

    /// Mark the blackout.
    pub fn detach(&mut self) {
        self.channel = OwnerChannel::Detached;
    }

    /// Is it the same as the entry system's request?
    ///
    /// ⚠️ Solution B-04: the result goes to **only** the request that started it. The number
    /// comes out from the login system, and it lies in context.
    pub fn answers(&self, transaction: crate::ports::TransactionId) -> bool {
        self.caller.answers(transaction)
    }

    /// Can the owner get the result?
    ///
    /// ⚠️ The disabled owner does not receive the result - but also **not ** cancels the
    /// change: its reconciliation remains the responsibility of the platform.
    pub fn may_receive(&self) -> bool {
        self.channel == OwnerChannel::Attached
    }
}

#[path = "owner_handle.tests.rs"]
#[cfg(test)]
mod tests;
