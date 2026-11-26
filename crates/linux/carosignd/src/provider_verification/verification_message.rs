use super::{ProgressObservation, VerificationObservation};

/// An event verified with respect to a command without a confidence assessment.
pub enum VerificationMessage {
    /// The worker took the command.
    Accepted,
    /// The worker refused to perform the command.
    Rejected(carosign_protocol::carosign::common::v1::Error),
    /// Operation progress.
    Progress(ProgressObservation),
    /// Checked observation, not yet appreciated by the inspector.
    Outcome {
        observation: Box<VerificationObservation>,
    },
}

impl core::fmt::Debug for VerificationMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Accepted => "Accepted",
            Self::Rejected(_) => "Rejected(..)",
            Self::Progress(_) => "Progress(..)",
            Self::Outcome { .. } => "Outcome(..)",
        })
    }
}
