use super::super::*;

#[test]
fn accepted_factor_is_success_without_a_switch() {
    // This is a code table check, not a service confirmation or a PAM call binding result.
    assert_eq!(to_code(Outcome::FactorAccepted), PamCode::Success);
}

#[test]
fn invalid_evidence_is_never_success() {
    assert_ne!(to_code(Outcome::InvalidEvidence), PamCode::Success);
    assert_eq!(to_code(Outcome::InvalidEvidence), PamCode::AuthInfoUnavail);
    assert_eq!(to_code(Outcome::Indeterminate), PamCode::AuthInfoUnavail);
}

#[test]
fn unavailable_allows_password_fallback() {
    assert_eq!(to_code(Outcome::Unavailable), PamCode::AuthInfoUnavail);
    assert_eq!(to_code(Outcome::DaemonError), PamCode::AuthInfoUnavail);
}

#[test]
fn raw_values_match_libpam() {
    assert_eq!(PamCode::Success.raw(), 0);
    assert_eq!(PamCode::AuthErr.raw(), 7);
    assert_eq!(PamCode::AuthInfoUnavail.raw(), 9);
    assert_eq!(PamCode::UserUnknown.raw(), 10);
}
