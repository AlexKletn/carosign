use core::fmt;
use core::hash::Hash;

/// What's wrong with the proof.
///
/// ⚠️ Taxonomy from document 11. All options are **opt-out: none turns into
/// `NotMatched`Because the meaning for the user and for security is different.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceError {
    /// The material is not being understood.
    MalformedEvidence,
    /// The format is not supported by the auditor.
    UnsupportedFormat,
    WrongRecipient,
    /// The challenge is wrong.
    ChallengeMismatch,
    Expired,
    /// Withdrew.
    Revoked,
    /// Origin is not established.
    UnverifiedProvenance,
    /// The guarantee class is not fit for purpose.
    IncompatibleAssurance,
    /// Checker's not available.
    VerifierUnavailable,
}

impl fmt::Debug for EvidenceError {
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
        })
    }
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "доказательство отвергнуто: {self:?}")
    }
}

impl core::error::Error for EvidenceError {}
