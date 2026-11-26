use super::super::*;

#[test]
fn verified_backend_outcomes_use_fprintd_status_names() {
    assert_eq!(verify_status(VerifyOutcome::Matched), "verify-match");
    assert_eq!(verify_status(VerifyOutcome::NoMatch), "verify-no-match");
    assert_eq!(
        verify_status(VerifyOutcome::Unavailable),
        "verify-unknown-error"
    );
}
