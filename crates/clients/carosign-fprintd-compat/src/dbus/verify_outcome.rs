/// Final verification outcome from a trusted backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// The factor was accepted for this operation and caller.
    Matched,
    MatchedFinger(String),
    NoMatch,
    Unavailable,
}

pub(super) fn verify_status(result: VerifyOutcome) -> &'static str {
    match result {
        VerifyOutcome::Matched | VerifyOutcome::MatchedFinger(_) => "verify-match",
        VerifyOutcome::NoMatch => "verify-no-match",
        VerifyOutcome::Unavailable => "verify-unknown-error",
    }
}

#[path = "verify_outcome.tests.rs"]
#[cfg(test)]
mod outcome_tests;
