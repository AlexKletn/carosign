use core::fmt;

/// Receipt of acceptance.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CancelReceipt {
    /// Request accepted; this is **not* confirmation of the stop.
    Requested,
    /// The operation has already been completed.
    AlreadyTerminal,
}

impl fmt::Debug for CancelReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Requested => "CancelReceipt::Requested",
            Self::AlreadyTerminal => "CancelReceipt::AlreadyTerminal",
        })
    }
}

impl CancelReceipt {
    /// Is the transaction completed on a single receipt?
    ///
    /// ⚠️ Always. `false` for `Requested`: the stop is confirmed only by a separate
    /// observation (`CleanupConfirmed`/`Quiesced`).
    pub fn terminal_by_receipt_alone(self) -> bool {
        matches!(self, Self::AlreadyTerminal)
    }
}
