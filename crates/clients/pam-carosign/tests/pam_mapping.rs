//! Conformity of outcomes to PAM codes (document 07).
//!
//! ⚠️ The table of four lines is checked in its entirety: it is the boundary between “not
//! allowed” and “failed to check”, and they should not be confused.

use pam_carosign::client::{AdapterError, Request};
use pam_carosign::conversation::Prompt;
use pam_carosign::result::{Outcome, PamCode};
use pam_carosign::to_code;

#[test]
fn outcome_mapping_matches_the_document() {
    assert_eq!(to_code(Outcome::FactorAccepted), PamCode::Success);
    assert_eq!(to_code(Outcome::NotMatched), PamCode::AuthErr);
    assert_eq!(to_code(Outcome::Cancelled), PamCode::AuthInfoUnavail);
    assert_eq!(to_code(Outcome::Unavailable), PamCode::AuthInfoUnavail);
    assert_eq!(to_code(Outcome::DaemonError), PamCode::AuthInfoUnavail);
    assert_eq!(to_code(Outcome::UnknownSubject), PamCode::UserUnknown);
    assert_eq!(to_code(Outcome::InvalidEvidence), PamCode::AuthInfoUnavail);
    assert_eq!(to_code(Outcome::Indeterminate), PamCode::AuthInfoUnavail);
}

#[test]
fn identity_of_request_is_checked_elsewhere_not_by_argument() {
    // ⚠️ B-04: The transaction number lives in context, not in the adapter’s arguments.
    use carosign_runtime::ports::TransactionId;
    assert!(TransactionId::from_bytes([0u8; 16]).is_none());
}

#[test]
fn password_fallback_stays_possible() {
    // ⚠️ “Unavailable” should leave a backup path: it is not a refusal.
    assert_eq!(to_code(Outcome::Unavailable), PamCode::AuthInfoUnavail);
    assert_ne!(to_code(Outcome::Unavailable), PamCode::AuthErr);
}

#[test]
fn request_shape_is_checked_but_never_authorizes() {
    let request = Request {
        user: "alex".into(),
        service: "login".into(),
        connection: "tty1".into(),
    };
    assert!(request.validate().is_ok());

    let empty = Request {
        user: String::new(),
        service: "login".into(),
        connection: "tty1".into(),
    };
    assert_eq!(empty.validate(), Err(AdapterError::EmptyUser));

    let control = Request {
        user: "alex\u{0007}".into(),
        service: "login".into(),
        connection: "tty1".into(),
    };
    assert_eq!(control.validate(), Err(AdapterError::ControlCharacters));
}

#[test]
fn prompts_come_from_events_not_from_provider_text() {
    let localizer = carosign_l10n::Localizer::system(carosign_l10n::Preferences::new("en", None));
    assert_eq!(
        Prompt::PlaceFinger.localized(&localizer),
        "Place your finger on the reader."
    );
    assert_eq!(
        Prompt::Retry.localized(&localizer),
        "Try scanning your finger again."
    );
}
