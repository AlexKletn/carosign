use core::fmt;

/// Why the reconciliation didn't set anything up.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReconciliationFailure {
    /// The provider does not know how to securely request (for example, Goodix without a
    /// confirmed status check).
    Unsupported,
    /// The request has been fulfilled, but the status has not been established.
    Unknown,
    /// You can't make out the answer.
    MalformedReply,
    /// The era of the device or its identity has changed.
    DeviceChanged,
    /// Checker's not available.
    VerifierUnavailable,
}

impl fmt::Debug for ReconciliationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "Unsupported",
            Self::Unknown => "Unknown",
            Self::MalformedReply => "MalformedReply",
            Self::DeviceChanged => "DeviceChanged",
            Self::VerifierUnavailable => "VerifierUnavailable",
        })
    }
}
