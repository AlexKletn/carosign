use carosign_protocol::carosign::provider::v1 as provider;

/// What does the worker think about the cancellation?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CancelState {
    /// We didn't.
    Idle,
    /// There's a request; the hardware can still work.
    Requested,
    /// The hardware is stopped (confirmed by those who can confirm it).
    Quiesced,
    /// Cancellation is not possible for this device.
    Unsupported,
}

impl CancelState {
    /// Accept the request for cancellation.
    pub fn request(self) -> Self {
        match self {
            Self::Idle => Self::Requested,
            other => other,
        }
    }

    /// Notice that the hardware has stopped.
    ///
    /// ⚠️ This can only be done by someone who has actually stopped; the SDK worker does
    /// not claim it.
    pub fn confirm_quiesced(self) -> Self {
        Self::Quiesced
    }

    /// Can the operation be considered a complete cancellation?
    pub fn is_quiesced(self) -> bool {
        matches!(self, Self::Quiesced)
    }

    /// Receipt for the wire.
    pub fn receipt(self) -> provider::CancelStatus {
        match self {
            // The rest alone does not prove that the outcome of the operation has already
            // been issued.
            Self::Idle | Self::Requested | Self::Quiesced => {
                provider::CancelStatus::CancelRequested
            }
            Self::Unsupported => provider::CancelStatus::CancelUnsupported,
        }
    }
}

#[path = "cancel_state.tests.rs"]
#[cfg(test)]
mod tests;
