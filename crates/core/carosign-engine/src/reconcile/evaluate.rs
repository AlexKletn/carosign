use super::ExpectedStates;
use super::ReconciliationFailure;
use super::ReconciliationVerdict;
use super::VerifiedState;

/// Evaluates available observation without giving incomplete comparisons as evidence.
///
/// Current arguments do not contain a complete snapshot of the area, its identity, and
/// independent verification of origin; according to document 28, such context can only give
/// an indefinite result, even if the identifier matches.
pub fn evaluate(
    _expected: ExpectedStates,
    _observed: Option<VerifiedState>,
    failure: Option<ReconciliationFailure>,
) -> ReconciliationVerdict {
    ReconciliationVerdict::Indeterminate(failure.unwrap_or(ReconciliationFailure::Unknown))
}
