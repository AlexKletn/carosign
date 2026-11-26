use core::fmt;

/// Type of refusal during verification (document 11, evidence errors).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerificationFailure {
    /// The evidence is understood, but incorrect.
    MalformedEvidence,
    /// The format is not supported by the auditor.
    UnsupportedFormat,
    /// Proof to the wrong recipient.
    WrongRecipient,
    /// Wrong task (challenge).
    ChallengeMismatch,
    Expired,
    /// Withdrew.
    Revoked,
    /// Origin is not established.
    UnverifiedProvenance,
    /// The guarantee class does not fit the requested purpose.
    IncompatibleAssurance,
    /// The examiner is unavailable - rejection, not condescension.
    VerifierUnavailable,
    /// Confirmed verification command **not* transmitted (B-02).
    ///
    /// ⚠️ This is a launch error, not a matching error: there was no biometric response at
    /// all.
    DispatchNotSent,
    /// It is not known if the command has completed the B-02.
    ///
    /// ⚠️ There is no success; the device may need to be rebuilt before the next operation.
    DispatchOutcomeUnknown,
}

impl fmt::Debug for VerificationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MalformedEvidence => "MalformedEvidence",
            Self::UnsupportedFormat => "UnsupportedFormat",
            Self::WrongRecipient => "WrongRecipient",
            Self::ChallengeMismatch => "ChallengeMismatch",
            Self::Expired => "Expired",
            Self::Revoked => "Revoked",
            Self::UnverifiedProvenance => "UnverifiedProvenance",
            Self::IncompatibleAssurance => "IncompatibleAssurance",
            Self::VerifierUnavailable => "VerifierUnavailable",
            Self::DispatchNotSent => "DispatchNotSent",
            Self::DispatchOutcomeUnknown => "DispatchOutcomeUnknown",
        })
    }
}
