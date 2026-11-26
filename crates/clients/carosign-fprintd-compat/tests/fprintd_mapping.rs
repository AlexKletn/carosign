//! Tests fprintd outcome mapping and scanner ownership.

use carosign_fprintd_compat::ownership::{ClaimOutcome, Owner, claim, release};
use carosign_fprintd_compat::translate::{
    FprintdResult, carosign_evidence_placeholder::OutcomeKind, from_platform,
};

#[test]
fn foreign_result_is_never_success() {
    assert_eq!(
        from_platform(Some(OutcomeKind::Accepted)),
        FprintdResult::Failed
    );
}

#[test]
fn unavailability_is_visible_not_hidden() {
    assert_eq!(from_platform(None), FprintdResult::Unavailable);
    assert_eq!(
        from_platform(Some(OutcomeKind::Unavailable)),
        FprintdResult::Unavailable
    );
}

#[test]
fn two_owners_are_impossible() {
    assert_eq!(
        claim(Owner::Platform, Owner::SystemFprintd),
        ClaimOutcome::Refused {
            by: Owner::Platform
        }
    );
    assert_eq!(release(Owner::Platform, Owner::Platform), Owner::None);
}
