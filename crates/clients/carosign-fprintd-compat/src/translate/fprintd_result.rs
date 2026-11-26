use super::carosign_evidence_placeholder;
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FprintdResult {
    Success,
    NoMatch,
    Failed,
    Unavailable,
    Cancelled,
}

///
/// A positive result requires verified request binding. A generic outcome is insufficient.
pub fn from_platform(
    outcome: Option<carosign_evidence_placeholder::OutcomeKind>,
) -> FprintdResult {
    use carosign_evidence_placeholder::OutcomeKind as K;
    match outcome {
        Some(K::Accepted) => FprintdResult::Failed,
        Some(K::NotMatched) => FprintdResult::NoMatch,
        Some(K::Cancelled) => FprintdResult::Cancelled,
        Some(K::Unavailable) | None => FprintdResult::Unavailable,
        Some(K::Failed) => FprintdResult::Failed,
    }
}

impl fmt::Display for FprintdResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Success => "успех",
            Self::NoMatch => "не совпало",
            Self::Failed => "отказ",
            Self::Unavailable => "нет устройства",
            Self::Cancelled => "отменено",
        })
    }
}
